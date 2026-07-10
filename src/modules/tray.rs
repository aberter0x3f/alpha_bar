use std::{
  collections::HashMap,
  fs::{self, File},
  io::Cursor,
  path::{Path, PathBuf},
  sync::mpsc::{self, Receiver, Sender},
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use cairo::{Format, ImageSurface};
use zbus::{
  MatchRule,
  blocking::{Connection, MessageIterator, Proxy},
  zvariant::{Array, OwnedObjectPath, OwnedValue, Structure, Value},
};

use crate::{
  runtime::{RuntimeCommand, Update},
  state::{Class, Click, Icon, Segment, Side, TrayMenuEntry, TrayMenuPopup},
};

pub fn spawn(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  thread::spawn(move || {
    let (pump_tx, pump_rx) = mpsc::channel();
    let cmd_tx = pump_tx.clone();
    thread::spawn(move || {
      while let Ok(command) = cmd_rx.recv() {
        if cmd_tx.send(PumpEvent::Command(command)).is_err() {
          break;
        }
      }
    });
    watch_tray(pump_tx.clone());
    let mut tray_generation = 0u64;
    let mut menu_generation = 0u64;
    schedule_segments(&pump_tx, tray_generation);
    while let Ok(event) = pump_rx.recv() {
      match event {
        PumpEvent::Refresh => {
          tray_generation += 1;
          schedule_segments(&pump_tx, tray_generation);
        }
        PumpEvent::Job(TrayJob::Segments(generation, segments))
          if generation == tray_generation =>
        {
          let _ = tx.send(Update::Tray(segments));
        }
        PumpEvent::Job(TrayJob::Menu(generation, menu)) if generation == menu_generation => {
          let _ = tx.send(Update::TrayMenu(Some(menu)));
        }
        PumpEvent::Job(TrayJob::MenuError(generation, address, error))
          if generation == menu_generation =>
        {
          eprintln!("tray menu failed for {address}: {error}");
        }
        PumpEvent::Job(_) => {}
        PumpEvent::Command(RuntimeCommand::TrayActivate {
          address,
          context_menu,
          x,
          y,
        }) => {
          if context_menu && let Some((path, item_theme_path)) = menu_address(&address) {
            menu_generation += 1;
            let generation = menu_generation;
            let _ = tx.send(Update::TrayMenu(Some(pending_menu(
              address.clone(),
              path.clone(),
              x,
            ))));
            let tx = pump_tx.clone();
            thread::spawn(move || {
              let result = refresh_tray_menu(address.clone(), path, item_theme_path, x)
                .map(|menu| TrayJob::Menu(generation, menu))
                .unwrap_or_else(|error| TrayJob::MenuError(generation, address, error.to_string()));
              let _ = tx.send(PumpEvent::Job(result));
            });
            continue;
          }
          thread::spawn(move || {
            let result = if context_menu {
              open_context_menu(&address, x, y)
            } else {
              activate(&address, x, y)
            };
            if let Err(error) = result {
              eprintln!("tray click failed for {address}: {error}");
            }
          });
        }
        PumpEvent::Command(RuntimeCommand::TrayMenuItem { address, path, id }) => {
          thread::spawn(move || {
            if let Err(error) = activate_menu_item(&address, &path, id) {
              eprintln!("tray menu item failed for {address}: {error}");
            }
          });
          let _ = tx.send(Update::TrayMenu(None));
        }
        PumpEvent::Command(RuntimeCommand::TrayMenuClose) => {
          let _ = tx.send(Update::TrayMenu(None));
        }
      }
    }
  });
}

enum PumpEvent {
  Refresh,
  Command(RuntimeCommand),
  Job(TrayJob),
}

enum TrayJob {
  Segments(u64, Vec<Segment>),
  Menu(u64, TrayMenuPopup),
  MenuError(u64, String, String),
}

#[derive(Clone, Default)]
struct TrayItem {
  icon_theme_path: Option<String>,
  icon_name: Option<String>,
  icon_pixmap: Vec<(i32, i32, Vec<u8>)>,
}

#[derive(Clone, Default)]
struct MenuItem {
  id: i32,
  label: Option<String>,
  enabled: bool,
  visible: bool,
  icon_name: Option<String>,
  icon_data: Option<Vec<u8>>,
  menu_type: MenuType,
  submenu: Vec<MenuItem>,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum MenuType {
  Separator,
  #[default]
  Standard,
}

impl From<&str> for MenuType {
  fn from(value: &str) -> Self {
    if value == "separator" {
      Self::Separator
    } else {
      Self::Standard
    }
  }
}

type DbusMenuLayout = (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>));

fn watch_tray(tx: Sender<PumpEvent>) {
  let _ = tx.send(PumpEvent::Refresh);
  watch_rule(&tx, "org.kde.StatusNotifierWatcher");
  watch_rule(&tx, "org.kde.StatusNotifierItem");
  watch_rule(&tx, "com.canonical.dbusmenu");
}

fn watch_rule(tx: &Sender<PumpEvent>, interface: &'static str) {
  let tx = tx.clone();
  thread::spawn(move || {
    let Ok(connection) = Connection::session() else {
      return;
    };
    let Ok(builder) = MatchRule::builder()
      .msg_type(zbus::message::Type::Signal)
      .interface(interface)
    else {
      return;
    };
    let rule = builder.build();
    let Ok(mut messages) = MessageIterator::for_match_rule(rule, &connection, Some(32)) else {
      return;
    };
    while let Some(message) = messages.next() {
      if message.is_ok() && tx.send(PumpEvent::Refresh).is_err() {
        return;
      }
    }
  });
}

fn activate_menu_item(address: &str, path: &str, id: i32) -> zbus::Result<()> {
  let (destination, _) = parse_address(address);
  let connection = Connection::session()?;
  let proxy = Proxy::new(&connection, destination, path, "com.canonical.dbusmenu")?;
  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_secs() as u32);
  proxy.call_method("Event", &(id, "clicked", Value::I32(0), timestamp))?;
  Ok(())
}

fn menu_address(address: &str) -> Option<(String, Option<String>)> {
  let (destination, _) = parse_address(address);
  let connection = Connection::session().ok()?;
  let proxy = Proxy::new(
    &connection,
    destination,
    parse_address(address).1,
    "org.kde.StatusNotifierItem",
  )
  .ok()?;
  let menu = proxy
    .get_property::<OwnedObjectPath>("Menu")
    .ok()?
    .to_string();
  let theme_path = proxy
    .get_property::<String>("IconThemePath")
    .ok()
    .filter(|path| !path.is_empty());
  Some((menu, theme_path))
}

fn refresh_tray_menu(
  address: String,
  path: String,
  item_theme_path: Option<String>,
  x: i32,
) -> zbus::Result<TrayMenuPopup> {
  let (destination, _) = parse_address(&address);
  let connection = Connection::session()?;
  let layout = {
    let proxy = Proxy::new(
      &connection,
      destination,
      path.as_str(),
      "com.canonical.dbusmenu",
    )?;
    let _: bool = proxy.call("AboutToShow", &0i32).unwrap_or(false);
    proxy.call::<_, _, DbusMenuLayout>("GetLayout", &(0i32, -1i32, Vec::<&str>::new()))?
  };
  let menu = menu_items_from_layout(layout);
  let xdg_data_roots = xdg_data_roots();
  let icon_theme = current_icon_theme();
  let entries = menu_entries(
    &menu,
    item_theme_path.as_deref().map(Path::new),
    &xdg_data_roots,
    &icon_theme,
  );
  Ok(TrayMenuPopup {
    address,
    path,
    x: x.max(0) as f64,
    y: crate::config::BAR.height as f64,
    entries,
    open_path: Vec::new(),
  })
}

fn pending_menu(address: String, path: String, x: i32) -> TrayMenuPopup {
  TrayMenuPopup {
    address,
    path,
    x: x.max(0) as f64,
    y: crate::config::BAR.height as f64,
    entries: vec![TrayMenuEntry {
      id: 0,
      label: "Loading...".into(),
      enabled: false,
      separator: false,
      icon: None,
      children: Vec::new(),
    }],
    open_path: Vec::new(),
  }
}

fn menu_items_from_layout(layout: DbusMenuLayout) -> Vec<MenuItem> {
  let (_, (_, _, submenus)) = layout;
  submenus.iter().filter_map(menu_item_from_value).collect()
}

fn menu_item_from_value(value: &OwnedValue) -> Option<MenuItem> {
  let structure = value.downcast_ref::<&Structure>().ok()?;
  let mut fields = structure.fields().iter();
  let mut menu = MenuItem {
    enabled: true,
    visible: true,
    ..Default::default()
  };
  if let Some(Value::I32(id)) = fields.next() {
    menu.id = *id;
  }
  if let Some(Value::Dict(dict)) = fields.next() {
    menu.label = dict
      .get::<&str, &str>(&"label")
      .ok()
      .flatten()
      .map(|label| label.replace('_', ""));
    menu.enabled = dict
      .get::<&str, bool>(&"enabled")
      .ok()
      .flatten()
      .unwrap_or(true);
    menu.visible = dict
      .get::<&str, bool>(&"visible")
      .ok()
      .flatten()
      .unwrap_or(true);
    menu.icon_name = dict
      .get::<&str, &str>(&"icon-name")
      .ok()
      .flatten()
      .map(str::to_owned);
    menu.icon_data = dict
      .get::<&str, &Array>(&"icon-data")
      .ok()
      .flatten()
      .and_then(|array| {
        array
          .iter()
          .map(|value| value.downcast_ref::<u8>().ok())
          .collect()
      });
    menu.menu_type = dict
      .get::<&str, &str>(&"type")
      .ok()
      .flatten()
      .map(MenuType::from)
      .unwrap_or_default();
  }
  if let Some(Value::Array(array)) = fields.next() {
    menu.submenu = array
      .iter()
      .filter_map(|value| OwnedValue::try_from(value).ok())
      .filter_map(|value| menu_item_from_value(&value))
      .collect();
  }
  Some(menu)
}

fn menu_entries(
  items: &[MenuItem],
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  icon_theme: &str,
) -> Vec<TrayMenuEntry> {
  items
    .iter()
    .filter_map(|item| menu_entry(item, item_theme_path, xdg_data_roots, icon_theme))
    .collect()
}

fn menu_entry(
  item: &MenuItem,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  icon_theme: &str,
) -> Option<TrayMenuEntry> {
  if !item.visible {
    return None;
  }
  let separator = item.menu_type == MenuType::Separator;
  let children = item
    .submenu
    .iter()
    .filter_map(|child| menu_entry(child, item_theme_path, xdg_data_roots, icon_theme))
    .collect();
  Some(TrayMenuEntry {
    id: item.id,
    label: item.label.clone().unwrap_or_default(),
    enabled: item.enabled,
    separator,
    icon: menu_item_icon(item, item_theme_path, xdg_data_roots, icon_theme),
    children,
  })
}

fn menu_item_icon(
  item: &MenuItem,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  icon_theme: &str,
) -> Option<Icon> {
  item
    .icon_name
    .as_deref()
    .and_then(|name| {
      find_icon_path(name, item_theme_path, xdg_data_roots, icon_theme)
        .and_then(|path| icon_from_path(&path))
    })
    .or_else(|| item.icon_data.as_deref().and_then(icon_from_png_data))
}

fn schedule_segments(tx: &Sender<PumpEvent>, generation: u64) {
  let tx = tx.clone();
  thread::spawn(move || {
    let segments = segments(tray_items());
    let _ = tx.send(PumpEvent::Job(TrayJob::Segments(generation, segments)));
  });
}

fn tray_items() -> Vec<(String, TrayItem)> {
  full_addresses()
    .into_iter()
    .filter_map(|address| query_item(&address).map(|item| (address, item)))
    .collect()
}

fn segments(items: Vec<(String, TrayItem)>) -> Vec<Segment> {
  let xdg_data_roots = xdg_data_roots();
  let icon_theme = current_icon_theme();
  items
    .into_iter()
    .filter_map(|(address, item)| {
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
            .iter()
            .max_by_key(|pixmap| pixmap.0 * pixmap.1)
            .and_then(|pixmap| icon_from_argb(pixmap.0, pixmap.1, &pixmap.2))
        })
        .map(|icon| Segment::icon(Side::Left, Class::Normal, icon).click(Click::Tray(address)))
    })
    .collect()
}

fn full_addresses() -> Vec<String> {
  let Ok(connection) = Connection::session() else {
    return Vec::new();
  };
  let Ok(proxy) = Proxy::new(
    &connection,
    "org.kde.StatusNotifierWatcher",
    "/StatusNotifierWatcher",
    "org.kde.StatusNotifierWatcher",
  ) else {
    return Vec::new();
  };
  sort_addresses(
    proxy
      .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
      .unwrap_or_default(),
  )
}

fn sort_addresses(mut addresses: Vec<String>) -> Vec<String> {
  addresses.sort();
  addresses
}

fn query_item(address: &str) -> Option<TrayItem> {
  let (destination, path) = parse_address(address);
  let connection = Connection::session().ok()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").ok()?;
  Some(TrayItem {
    icon_theme_path: proxy
      .get_property::<String>("IconThemePath")
      .ok()
      .filter(|path| !path.is_empty()),
    icon_name: proxy
      .get_property::<String>("IconName")
      .ok()
      .filter(|name| !name.is_empty()),
    icon_pixmap: proxy
      .get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap")
      .unwrap_or_default(),
  })
}

fn open_context_menu(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_address(address);
  let connection = Connection::session()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem")?;
  proxy.call_method("ContextMenu", &(x, y))?;
  Ok(())
}

fn activate(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_address(address);
  let connection = Connection::session()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem")?;
  proxy.call_method("Activate", &(x, y))?;
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
  icon_from_surface(surface)
}

fn icon_from_png_data(data: &[u8]) -> Option<Icon> {
  let mut cursor = Cursor::new(data);
  let surface = ImageSurface::create_from_png(&mut cursor).ok()?;
  icon_from_surface(surface)
}

fn icon_from_surface(surface: ImageSurface) -> Option<Icon> {
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

  #[test]
  fn menu_tree() {
    let menu = vec![MenuItem {
      id: 1,
      label: Some("Parent".into()),
      enabled: true,
      visible: true,
      submenu: vec![MenuItem {
        id: 2,
        label: Some("Child".into()),
        enabled: true,
        visible: true,
        ..Default::default()
      }],
      ..Default::default()
    }];

    let entries = menu_entries(&menu, None, &[], "hicolor");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "Parent");
    assert_eq!(entries[0].children[0].label, "Child");
  }

  #[test]
  fn tray_order() {
    let addresses = vec![":1.9/Z".into(), ":1.2/A".into(), ":1.10/B".into()];

    assert_eq!(
      sort_addresses(addresses),
      vec![":1.10/B", ":1.2/A", ":1.9/Z"]
    );
  }

  #[test]
  fn loading_menu() {
    let menu = pending_menu(":1.2/StatusNotifierItem".into(), "/menu".into(), 12);

    assert_eq!(menu.entries.len(), 1);
    assert_eq!(menu.entries[0].label, "Loading...");
    assert!(!menu.entries[0].enabled);
  }
}
