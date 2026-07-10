use std::{
  collections::HashMap,
  fs::{self, File},
  path::{Path, PathBuf},
  sync::mpsc::{Receiver, Sender},
  thread,
  time::Duration,
};

use cairo::{Format, ImageSurface};

use crate::{
  runtime::{RuntimeCommand, Update},
  state::{Class, Click, Icon, Segment, Side},
};

pub fn spawn(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  thread::spawn(move || {
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
    else {
      return;
    };
    rt.block_on(async move {
      let Ok(client) = system_tray::client::Client::new().await else {
        return;
      };
      let mut events = client.subscribe();
      let _ = tx.send(Update::Tray(segments(&client).await));
      loop {
        tokio::select! {
          event = events.recv() => {
            if event.is_err() { return; }
            let _ = tx.send(Update::Tray(segments(&client).await));
          }
          _ = tokio::time::sleep(Duration::from_millis(50)) => {
            while let Ok(command) = cmd_rx.try_recv() {
              match command {
                RuntimeCommand::TrayActivate { address, context_menu, x, y } => {
                  if context_menu {
                    let _ = open_context_menu(&address, x, y).await;
                  } else {
                    let _ = activate(&address, x, y).await;
                  }
                }
              }
            }
          }
        }
      }
    });
  });
}

async fn segments(client: &system_tray::client::Client) -> Vec<Segment> {
  let items = client.items();
  let Ok(items) = items.lock() else {
    return Vec::new();
  };
  let xdg_data_roots = xdg_data_roots();
  let icon_theme = current_icon_theme();
  let addresses = full_addresses().await;
  items
    .iter()
    .filter_map(|(address, (item, _))| {
      let click_address = addresses
        .get(address.as_str())
        .cloned()
        .unwrap_or_else(|| address.clone());
      item
        .icon_name
        .as_deref()
        .and_then(|name| {
          find_icon_path(
            name,
            item.icon_theme_path.as_deref().map(Path::new),
            &xdg_data_roots,
            &icon_theme,
          )
          .and_then(|path| icon_from_path(&path))
        })
        .or_else(|| {
          item
            .icon_pixmap
            .as_ref()
            .and_then(|pixmaps| {
              pixmaps
                .iter()
                .max_by_key(|pixmap| pixmap.width * pixmap.height)
            })
            .and_then(|pixmap| icon_from_argb(pixmap.width, pixmap.height, &pixmap.pixels))
        })
        .map(|icon| {
          Segment::icon(Side::Left, Class::Normal, icon).click(Click::Tray(click_address))
        })
    })
    .collect()
}

async fn full_addresses() -> HashMap<String, String> {
  let Ok(connection) = zbus::Connection::session().await else {
    return HashMap::new();
  };
  let Ok(proxy) = zbus::Proxy::new(
    &connection,
    "org.kde.StatusNotifierWatcher",
    "/StatusNotifierWatcher",
    "org.kde.StatusNotifierWatcher",
  )
  .await
  else {
    return HashMap::new();
  };
  proxy
    .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|address| (parse_address(&address).0.to_owned(), address))
    .collect()
}

async fn open_context_menu(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_address(address);
  let connection = zbus::Connection::session().await?;
  let proxy =
    zbus::Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").await?;
  proxy.call_method("ContextMenu", &(x, y)).await?;
  Ok(())
}

async fn activate(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_address(address);
  let connection = zbus::Connection::session().await?;
  let proxy =
    zbus::Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").await?;
  proxy.call_method("Activate", &(x, y)).await?;
  Ok(())
}

fn parse_address(address: &str) -> (&str, String) {
  address.split_once('/').map_or(
    (address, "/StatusNotifierItem".into()),
    |(destination, path)| (destination, format!("/{path}")),
  )
}

fn current_icon_theme() -> String {
  let dirs = config_dirs();
  dirs
    .iter()
    .find_map(|dir| read_icon_theme(&dir.join("gtk-3.0/settings.ini")))
    .or_else(|| {
      dirs
        .iter()
        .find_map(|dir| read_icon_theme(&dir.join("gtk-4.0/settings.ini")))
    })
    .or_else(|| {
      dirs
        .iter()
        .find_map(|dir| read_kde_icon_theme(&dir.join("kdeglobals")))
    })
    .unwrap_or_else(|| "hicolor".into())
}

fn config_dirs() -> Vec<PathBuf> {
  if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
    vec![PathBuf::from(dir)]
  } else if let Some(home) = std::env::var_os("HOME") {
    vec![PathBuf::from(home).join(".config")]
  } else {
    Vec::new()
  }
}

fn read_icon_theme(path: &Path) -> Option<String> {
  fs::read_to_string(path)
    .ok()?
    .lines()
    .map(str::trim)
    .find_map(|line| {
      line
        .strip_prefix("gtk-icon-theme-name=")
        .map(str::trim)
        .map(str::to_owned)
    })
    .filter(|name| !name.is_empty())
}

fn read_kde_icon_theme(path: &Path) -> Option<String> {
  let mut in_icons = false;
  for line in fs::read_to_string(path).ok()?.lines().map(str::trim) {
    if line.starts_with('[') && line.ends_with(']') {
      in_icons = &line[1..line.len() - 1] == "Icons";
    } else if in_icons
      && let Some(theme) = line.strip_prefix("Theme=").map(str::trim)
      && !theme.is_empty()
    {
      return Some(theme.to_owned());
    }
  }
  None
}

fn xdg_data_roots() -> Vec<PathBuf> {
  let mut roots = vec![PathBuf::from("/run/current-system/sw/share")];
  if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
    roots.push(PathBuf::from(home));
  } else if let Some(home) = std::env::var_os("HOME") {
    roots.push(PathBuf::from(home).join(".local/share"));
  }
  if let Some(dirs) = std::env::var_os("XDG_DATA_DIRS") {
    roots.extend(std::env::split_paths(&dirs));
  }
  roots
}

fn icon_from_argb(width: i32, height: i32, argb: &[u8]) -> Option<Icon> {
  let width = usize::try_from(width).ok()?;
  let height = usize::try_from(height).ok()?;
  if argb.len() != width * height * 4 {
    return None;
  }
  let mut pixels = Vec::with_capacity(argb.len());
  for px in argb.chunks_exact(4) {
    pixels.extend([px[3], px[2], px[1], px[0]]);
  }
  Some(Icon {
    width,
    height,
    pixels,
  })
}

fn icon_from_png(path: &Path) -> Option<Icon> {
  let mut file = File::open(path).ok()?;
  let surface = ImageSurface::create_from_png(&mut file).ok()?;
  let width = usize::try_from(surface.width()).ok()?;
  let height = usize::try_from(surface.height()).ok()?;
  let stride = usize::try_from(surface.stride()).ok()?;
  if surface.format() != Format::ARgb32 {
    return None;
  }
  let mut pixels = Vec::with_capacity(width * height * 4);
  surface
    .with_data(|data| {
      for row in 0..height {
        let start = row * stride;
        pixels.extend_from_slice(&data[start..start + width * 4]);
      }
    })
    .ok()?;
  Some(Icon {
    width,
    height,
    pixels,
  })
}

fn icon_from_path(path: &Path) -> Option<Icon> {
  match path.extension().and_then(|extension| extension.to_str()) {
    Some("png") => icon_from_png(path),
    Some("svg") => icon_from_svg(path),
    _ => None,
  }
}

fn icon_from_svg(path: &Path) -> Option<Icon> {
  let mut options = resvg::usvg::Options {
    resources_dir: path.parent().map(Path::to_path_buf),
    ..resvg::usvg::Options::default()
  };
  options.fontdb_mut().load_system_fonts();
  let data = fs::read(path).ok()?;
  let tree = resvg::usvg::Tree::from_data(&data, &options).ok()?;
  let size = tree.size().to_int_size();
  let width = usize::try_from(size.width()).ok()?;
  let height = usize::try_from(size.height()).ok()?;
  let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
  resvg::render(
    &tree,
    resvg::tiny_skia::Transform::default(),
    &mut pixmap.as_mut(),
  );
  let mut pixels = Vec::with_capacity(width * height * 4);
  for px in pixmap.data().chunks_exact(4) {
    pixels.extend([px[2], px[1], px[0], px[3]]);
  }
  Some(Icon {
    width,
    height,
    pixels,
  })
}

fn find_icon_path(
  icon_name: &str,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  theme: &str,
) -> Option<PathBuf> {
  let icon_roots = icon_roots(item_theme_path, xdg_data_roots);
  lookup_theme(icon_name, 20, 1, theme, &icon_roots, &mut Vec::new())
    .or_else(|| {
      (theme != "hicolor")
        .then(|| lookup_theme(icon_name, 20, 1, "hicolor", &icon_roots, &mut Vec::new()))
        .flatten()
    })
    .or_else(|| lookup_fallback(icon_name, item_theme_path, xdg_data_roots))
}

fn icon_roots(item_theme_path: Option<&Path>, xdg_data_roots: &[PathBuf]) -> Vec<PathBuf> {
  let mut roots = item_theme_path
    .into_iter()
    .map(Path::to_path_buf)
    .collect::<Vec<_>>();
  roots.push(home_icons_dir());
  roots.extend(xdg_data_roots.iter().map(|root| root.join("icons")));
  roots
}

fn home_icons_dir() -> PathBuf {
  std::env::var_os("HOME").map_or_else(
    || PathBuf::from(".icons"),
    |home| PathBuf::from(home).join(".icons"),
  )
}

fn lookup_theme(
  icon: &str,
  size: u32,
  scale: u32,
  theme: &str,
  roots: &[PathBuf],
  seen: &mut Vec<String>,
) -> Option<PathBuf> {
  if seen.iter().any(|name| name == theme) {
    return None;
  }
  seen.push(theme.to_owned());
  let theme_dirs = theme_dirs(theme, roots)?;
  lookup_in_theme_dirs(icon, size, scale, &theme_dirs, true)
    .or_else(|| lookup_in_theme_dirs(icon, size, scale, &theme_dirs, false))
    .or_else(|| {
      theme_dirs
        .inherits
        .iter()
        .find_map(|parent| lookup_theme(icon, size, scale, parent, roots, seen))
    })
}

struct ThemeDirs {
  bases: Vec<PathBuf>,
  dirs: Vec<IconDir>,
  inherits: Vec<String>,
}

struct IconDir {
  name: String,
  size: u32,
  scale: u32,
  min: u32,
  max: u32,
  threshold: u32,
  kind: DirKind,
}

enum DirKind {
  Fixed,
  Scalable,
  Threshold,
}

fn theme_dirs(theme: &str, roots: &[PathBuf]) -> Option<ThemeDirs> {
  let bases = roots
    .iter()
    .map(|root| root.join(theme))
    .filter(|path| path.is_dir())
    .collect::<Vec<_>>();
  let index = bases
    .iter()
    .map(|base| base.join("index.theme"))
    .find(|path| path.exists())?;
  let text = fs::read_to_string(index).ok()?;
  let dirs = key(&text, "Icon Theme", "Directories")
    .into_iter()
    .chain(key(&text, "Icon Theme", "ScaledDirectories"))
    .flat_map(|value| split_list(&value))
    .filter_map(|name| icon_dir(&text, &name))
    .collect();
  Some(ThemeDirs {
    bases,
    dirs,
    inherits: key(&text, "Icon Theme", "Inherits").map_or_else(Vec::new, |v| split_list(&v)),
  })
}

fn icon_dir(text: &str, name: &str) -> Option<IconDir> {
  let size = key(text, name, "Size")?.parse().ok()?;
  let scale = key(text, name, "Scale")
    .and_then(|v| v.parse().ok())
    .unwrap_or(1);
  let kind = match key(text, name, "Type").as_deref() {
    Some("Fixed") => DirKind::Fixed,
    Some("Scalable") => DirKind::Scalable,
    _ => DirKind::Threshold,
  };
  Some(IconDir {
    name: name.to_owned(),
    size,
    scale,
    min: key(text, name, "MinSize")
      .and_then(|v| v.parse().ok())
      .unwrap_or(size),
    max: key(text, name, "MaxSize")
      .and_then(|v| v.parse().ok())
      .unwrap_or(size),
    threshold: key(text, name, "Threshold")
      .and_then(|v| v.parse().ok())
      .unwrap_or(2),
    kind,
  })
}

fn lookup_in_theme_dirs(
  icon: &str,
  size: u32,
  scale: u32,
  theme: &ThemeDirs,
  exact: bool,
) -> Option<PathBuf> {
  let mut best = None;
  for dir in &theme.dirs {
    if exact && !dir_matches(dir, size, scale) {
      continue;
    }
    for base in &theme.bases {
      let Some(path) = icon_file(&base.join(&dir.name), icon) else {
        continue;
      };
      if exact {
        return Some(path);
      }
      let distance = dir_distance(dir, size, scale);
      if best.as_ref().is_none_or(|(best, _)| distance < *best) {
        best = Some((distance, path));
      }
    }
  }
  best.map(|(_, path)| path)
}

fn dir_matches(dir: &IconDir, size: u32, scale: u32) -> bool {
  if dir.scale != scale {
    return false;
  }
  match dir.kind {
    DirKind::Fixed => dir.size == size,
    DirKind::Scalable => dir.min <= size && size <= dir.max,
    DirKind::Threshold => {
      dir.size.saturating_sub(dir.threshold) <= size && size <= dir.size + dir.threshold
    }
  }
}

fn dir_distance(dir: &IconDir, size: u32, scale: u32) -> u32 {
  let icon = size * scale;
  match dir.kind {
    DirKind::Fixed => dir.size.abs_diff(icon),
    DirKind::Scalable if icon < dir.min * dir.scale => dir.min * dir.scale - icon,
    DirKind::Scalable if icon > dir.max * dir.scale => icon - dir.max * dir.scale,
    DirKind::Scalable => 0,
    DirKind::Threshold if icon < dir.size.saturating_sub(dir.threshold) * dir.scale => {
      dir.size.saturating_sub(dir.threshold) * dir.scale - icon
    }
    DirKind::Threshold if icon > (dir.size + dir.threshold) * dir.scale => {
      icon - (dir.size + dir.threshold) * dir.scale
    }
    DirKind::Threshold => 0,
  }
}

fn icon_file(dir: &Path, icon: &str) -> Option<PathBuf> {
  ["png", "svg"]
    .iter()
    .map(|ext| dir.join(format!("{icon}.{ext}")))
    .find(|path| path.exists())
}

fn lookup_fallback(
  icon: &str,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
) -> Option<PathBuf> {
  item_theme_path
    .and_then(|root| icon_file(root, icon))
    .or_else(|| {
      xdg_data_roots
        .iter()
        .find_map(|root| icon_file(&root.join("pixmaps"), icon))
    })
}

fn key(text: &str, section: &str, name: &str) -> Option<String> {
  let mut in_section = false;
  for line in text.lines().map(str::trim) {
    if line.starts_with('[') && line.ends_with(']') {
      in_section = &line[1..line.len() - 1] == section;
      continue;
    }
    if in_section
      && let Some((k, v)) = line.split_once('=')
      && k.trim() == name
    {
      return Some(v.trim().to_owned());
    }
  }
  None
}

fn split_list(value: &str) -> Vec<String> {
  value
    .split(',')
    .map(str::trim)
    .filter(|item| !item.is_empty())
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  #[test]
  fn argb_icon_to_bgra() {
    let icon = icon_from_argb(1, 1, &[0xff, 0xff, 0x00, 0x00]).unwrap();
    assert_eq!(icon.pixels, [0x00, 0x00, 0xff, 0xff]);
  }

  #[test]
  fn item_theme_png() {
    let root = std::env::temp_dir().join(format!("alpha-bar-tray-test-{}", std::process::id()));
    let item_theme = root.join("item-theme");
    let xdg_theme = root.join("xdg/icons/hicolor");
    let item_icon = item_theme.join("hicolor/24x24/status/blueman-action.png");
    let xdg_icon = xdg_theme.join("24x24/status/blueman-action.png");

    fs::create_dir_all(item_icon.parent().unwrap()).unwrap();
    fs::create_dir_all(xdg_icon.parent().unwrap()).unwrap();
    fs::write(
      item_theme.join("hicolor/index.theme"),
      "[Icon Theme]\nDirectories=24x24/status\n[24x24/status]\nSize=24\nType=Fixed\n",
    )
    .unwrap();
    fs::write(
      xdg_theme.join("index.theme"),
      "[Icon Theme]\nDirectories=24x24/status\n[24x24/status]\nSize=24\nType=Fixed\n",
    )
    .unwrap();
    fs::write(&item_icon, []).unwrap();
    fs::write(&xdg_icon, []).unwrap();

    assert_eq!(
      find_icon_path(
        "blueman-action",
        Some(&item_theme),
        &[root.join("xdg")],
        "hicolor",
      ),
      Some(item_icon)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn svg_icon() {
    let root = std::env::temp_dir().join(format!("alpha-bar-tray-svg-test-{}", std::process::id()));
    let icon = root.join("hicolor/scalable/status/blueman-action.svg");
    fs::create_dir_all(icon.parent().unwrap()).unwrap();
    fs::write(
      root.join("hicolor/index.theme"),
      "[Icon Theme]\nDirectories=scalable/status\n[scalable/status]\nSize=16\nType=Scalable\nMinSize=1\nMaxSize=64\n",
    )
    .unwrap();
    fs::write(
      &icon,
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#ff0000"/></svg>"##,
    )
    .unwrap();

    let path = find_icon_path("blueman-action", Some(&root), &[], "hicolor").unwrap();
    let rendered = icon_from_path(&path).unwrap();

    assert_eq!(path, icon);
    assert_eq!(rendered.width, 2);
    assert_eq!(rendered.height, 1);
    assert_eq!(rendered.pixels, [0, 0, 255, 255, 0, 0, 255, 255]);

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn theme_dirs() {
    let root =
      std::env::temp_dir().join(format!("alpha-bar-tray-theme-test-{}", std::process::id()));
    let selected = root.join("share/icons/Selected/symbolic/status/input-keyboard-symbolic.svg");
    let unselected = root.join("share/icons/Other/symbolic/status/input-keyboard-symbolic.svg");
    fs::create_dir_all(selected.parent().unwrap()).unwrap();
    fs::create_dir_all(unselected.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/Selected/index.theme"),
      "[Icon Theme]\nDirectories=symbolic/status\n[symbolic/status]\nSize=16\nType=Scalable\nMinSize=1\nMaxSize=64\n",
    )
    .unwrap();
    fs::write(
      root.join("share/icons/Other/index.theme"),
      "[Icon Theme]\nDirectories=symbolic/status\n[symbolic/status]\nSize=16\nType=Scalable\nMinSize=1\nMaxSize=64\n",
    )
    .unwrap();
    fs::write(&selected, []).unwrap();
    fs::write(&unselected, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard-symbolic",
        None,
        &[root.join("share")],
        "Selected",
      ),
      Some(selected)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn activation_path() {
    assert_eq!(
      parse_address(":1.58/StatusNotifierItem"),
      (":1.58", "/StatusNotifierItem".into())
    );
    assert_eq!(
      parse_address(":1.59/org/ayatana/NotificationItem/nm_applet"),
      (":1.59", "/org/ayatana/NotificationItem/nm_applet".into())
    );
  }
}
