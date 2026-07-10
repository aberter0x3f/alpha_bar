use std::{
  collections::HashMap,
  fs::{self, File},
  io::Cursor,
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock},
  time::SystemTime,
};

use cairo::{Format, ImageSurface};

use crate::state::Icon;

const TRAY_ICON_LOGICAL_SIZE: u32 = 20;
const ICON_RENDER_CACHE_MAX_BYTES: usize = 512 * 1024;
const ICON_EXTENSIONS: &[&str] = &["svg", "png"];

pub(super) fn tray_icon_size(scale: u32) -> u32 {
  TRAY_ICON_LOGICAL_SIZE * scale.max(1)
}

pub(super) fn current_icon_theme() -> String {
  let dirs = config_dirs();
  dirs
    .iter()
    .find_map(|dir| read_icon_theme(&dir.join("gtk-4.0/settings.ini")))
    .or_else(|| {
      dirs
        .iter()
        .find_map(|dir| read_icon_theme(&dir.join("gtk-3.0/settings.ini")))
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

pub(super) fn xdg_data_roots() -> Vec<PathBuf> {
  let mut roots = Vec::new();
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

pub(super) fn icon_from_png_data(data: &[u8]) -> Option<Icon> {
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

pub(super) fn icon_from_path(path: &Path, icon_size: u32) -> Option<Icon> {
  let metadata = fs::metadata(path).ok()?;
  let modified = metadata.modified().ok();
  let len = metadata.len();
  let key = IconRenderKey {
    path: path.to_path_buf(),
    icon_size,
  };
  if let Some(icon) = cached_render_icon(&key, modified, len) {
    return Some(icon);
  }
  let icon = match path.extension().and_then(|extension| extension.to_str()) {
    Some("png") => icon_from_png(path),
    Some("svg") => icon_from_svg(path, icon_size),
    _ => None,
  }?;
  insert_render_icon(key, modified, len, icon.clone());
  Some(icon)
}

fn icon_from_svg(path: &Path, icon_size: u32) -> Option<Icon> {
  let options = resvg::usvg::Options {
    resources_dir: path.parent().map(Path::to_path_buf),
    ..Default::default()
  };
  let data = fs::read(path).ok()?;
  let tree = resvg::usvg::Tree::from_data(&data, &options).ok()?;
  let size = tree.size().to_int_size();
  if size.height() == 0 {
    return None;
  }
  let scale = icon_size as f32 / size.height() as f32;
  let width = (size.width() as f32 * scale).round().max(1.0) as u32;
  let height = icon_size;
  let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
  resvg::render(
    &tree,
    resvg::tiny_skia::Transform::from_scale(scale, scale),
    &mut pixmap.as_mut(),
  );
  let width = usize::try_from(width).ok()?;
  let height = usize::try_from(height).ok()?;
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

pub(super) fn find_icon_path(
  icon_name: &str,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  theme: &str,
  icon_size: u32,
) -> Option<PathBuf> {
  let key = IconPathKey {
    name: icon_name.to_owned(),
    xdg_data_roots: xdg_data_roots.to_vec(),
    theme: theme.to_owned(),
    icon_size,
  };
  let cached_path = if item_theme_path.is_none() {
    icon_path_cache().lock().ok()?.get(&key).cloned()
  } else {
    None
  };
  if let Some(path) = cached_path {
    if path.exists() {
      return Some(path);
    }
    let _ = icon_path_cache().lock().ok()?.remove(&key);
  }
  let icon_roots = icon_roots(item_theme_path, xdg_data_roots);
  let path = lookup_theme(icon_name, icon_size, theme, &icon_roots, &mut Vec::new())
    .or_else(|| {
      (theme != "hicolor")
        .then(|| {
          lookup_theme(
            icon_name,
            icon_size,
            "hicolor",
            &icon_roots,
            &mut Vec::new(),
          )
        })
        .flatten()
    })
    .or_else(|| lookup_fallback(icon_name, item_theme_path, xdg_data_roots));
  if item_theme_path.is_none()
    && let Some(path) = &path
  {
    icon_path_cache().lock().ok()?.insert(key, path.clone());
  }
  path
}

pub(super) fn icon_from_name(
  name: &str,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  icon_theme: &str,
  icon_size: u32,
) -> Option<Icon> {
  find_icon_path(name, item_theme_path, xdg_data_roots, icon_theme, icon_size)
    .and_then(|path| icon_from_path(&path, icon_size))
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct IconPathKey {
  name: String,
  xdg_data_roots: Vec<PathBuf>,
  theme: String,
  icon_size: u32,
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct IconRenderKey {
  path: PathBuf,
  icon_size: u32,
}

#[derive(Clone)]
struct CachedIcon {
  modified: Option<SystemTime>,
  len: u64,
  icon: Icon,
  last_used: u64,
}

static ICON_PATH_CACHE: OnceLock<Mutex<HashMap<IconPathKey, PathBuf>>> = OnceLock::new();
static ICON_RENDER_CACHE: OnceLock<Mutex<HashMap<IconRenderKey, CachedIcon>>> = OnceLock::new();
static ICON_RENDER_TICK: OnceLock<Mutex<u64>> = OnceLock::new();

fn icon_path_cache() -> &'static Mutex<HashMap<IconPathKey, PathBuf>> {
  ICON_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn icon_render_cache() -> &'static Mutex<HashMap<IconRenderKey, CachedIcon>> {
  ICON_RENDER_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_render_tick() -> u64 {
  let mut tick = ICON_RENDER_TICK
    .get_or_init(|| Mutex::new(0))
    .lock()
    .expect("lock icon render tick");
  *tick += 1;
  *tick
}

fn cached_render_icon(key: &IconRenderKey, modified: Option<SystemTime>, len: u64) -> Option<Icon> {
  let mut cache = icon_render_cache().lock().ok()?;
  let Some(cached) = cache.get_mut(key) else {
    return None;
  };
  if cached.modified != modified || cached.len != len {
    cache.remove(key);
    return None;
  }
  cached.last_used = next_render_tick();
  Some(cached.icon.clone())
}

fn insert_render_icon(key: IconRenderKey, modified: Option<SystemTime>, len: u64, icon: Icon) {
  let Ok(mut cache) = icon_render_cache().lock() else {
    return;
  };
  cache.insert(
    key,
    CachedIcon {
      modified,
      len,
      icon,
      last_used: next_render_tick(),
    },
  );
  evict_render_cache(&mut cache);
}

fn render_cache_bytes(cache: &HashMap<IconRenderKey, CachedIcon>) -> usize {
  cache.values().map(|cached| cached.icon.pixels.len()).sum()
}

fn evict_render_cache(cache: &mut HashMap<IconRenderKey, CachedIcon>) {
  while render_cache_bytes(cache) > ICON_RENDER_CACHE_MAX_BYTES {
    let Some(key) = cache
      .iter()
      .min_by_key(|(_, cached)| cached.last_used)
      .map(|(key, _)| key.clone())
    else {
      break;
    };
    cache.remove(&key);
  }
}

fn icon_roots(item_theme_path: Option<&Path>, xdg_data_roots: &[PathBuf]) -> Vec<PathBuf> {
  let mut roots = item_theme_path
    .into_iter()
    .map(Path::to_path_buf)
    .collect::<Vec<_>>();
  roots.extend(xdg_data_roots.iter().map(|root| root.join("icons")));
  roots
}

fn lookup_theme(
  icon: &str,
  size: u32,
  theme: &str,
  roots: &[PathBuf],
  seen: &mut Vec<String>,
) -> Option<PathBuf> {
  if seen.iter().any(|name| name == theme) {
    return None;
  }
  seen.push(theme.to_owned());
  let bases = roots
    .iter()
    .map(|root| root.join(theme))
    .filter(|path| path.is_dir())
    .collect::<Vec<_>>();
  let index = theme_index(theme, roots, &bases);
  index
    .as_ref()
    .and_then(|index| lookup_index_dirs(icon, size, index))
    .or_else(|| {
      index
        .as_ref()
        .into_iter()
        .flat_map(|index| index.inherits.iter())
        .find_map(|parent| lookup_theme(icon, size, &parent, roots, seen))
    })
}

#[derive(Clone)]
struct ThemeIndex {
  bases: Vec<PathBuf>,
  dirs: Vec<ThemeDir>,
  inherits: Vec<String>,
}

#[derive(Clone)]
struct ThemeDir {
  name: String,
  size: u32,
  scale: u32,
}

fn lookup_index_dirs(icon: &str, size: u32, index: &ThemeIndex) -> Option<PathBuf> {
  index
    .dirs
    .iter()
    .filter_map(|dir| {
      index
        .bases
        .iter()
        .find_map(|base| icon_file(&base.join(&dir.name), icon))
        .map(|path| (size_score(dir.size * dir.scale, size), path))
    })
    .min_by_key(|(score, _)| *score)
    .map(|(_, path)| path)
}

fn theme_index(theme: &str, roots: &[PathBuf], bases: &[PathBuf]) -> Option<ThemeIndex> {
  let cache_key = ThemeIndexKey {
    theme: theme.to_owned(),
    roots: roots.to_vec(),
  };
  let path = bases
    .iter()
    .map(|base| base.join("index.theme"))
    .find(|path| path.exists())?;
  let metadata = fs::metadata(&path).ok()?;
  let modified = metadata.modified().ok();
  let len = metadata.len();
  if let Some(cached) = theme_index_cache().lock().ok()?.get(&cache_key).cloned()
    && cached.path == path
    && cached.modified == modified
    && cached.len == len
    && cached.index.bases == bases
  {
    return Some(cached.index);
  }
  let text = fs::read_to_string(&path).ok()?;
  let dirs = index_value(&text, "Icon Theme", "Directories")
    .into_iter()
    .chain(index_value(&text, "Icon Theme", "ScaledDirectories"))
    .flat_map(split_list)
    .map(|name| ThemeDir {
      size: theme_dir_number(&text, &name, "Size"),
      scale: theme_dir_number(&text, &name, "Scale").max(1),
      name,
    })
    .collect();
  let index = ThemeIndex {
    bases: bases.to_vec(),
    dirs,
    inherits: index_value(&text, "Icon Theme", "Inherits").map_or_else(Vec::new, split_list),
  };
  theme_index_cache().lock().ok()?.insert(
    cache_key,
    CachedThemeIndex {
      path,
      modified,
      len,
      index: index.clone(),
    },
  );
  Some(index)
}

#[derive(Clone, Hash, Eq, PartialEq)]
struct ThemeIndexKey {
  theme: String,
  roots: Vec<PathBuf>,
}

#[derive(Clone)]
struct CachedThemeIndex {
  path: PathBuf,
  modified: Option<SystemTime>,
  len: u64,
  index: ThemeIndex,
}

static THEME_INDEX_CACHE: OnceLock<Mutex<HashMap<ThemeIndexKey, CachedThemeIndex>>> =
  OnceLock::new();

fn theme_index_cache() -> &'static Mutex<HashMap<ThemeIndexKey, CachedThemeIndex>> {
  THEME_INDEX_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn theme_dir_number(index: &str, name: &str, key: &str) -> u32 {
  index_value(index, name, key)
    .and_then(|value| value.parse().ok())
    .or_else(|| {
      let first = name.split('/').next()?;
      if key == "Scale" {
        return first.split_once('@')?.1.split_once('x')?.0.parse().ok();
      }
      first.split('@').next()?.parse().ok()
    })
    .unwrap_or(if key == "Scale" {
      1
    } else {
      TRAY_ICON_LOGICAL_SIZE
    })
}

fn index_value<'a>(text: &'a str, section: &str, name: &str) -> Option<&'a str> {
  let mut current = "";
  for line in text.lines().map(str::trim) {
    if line.starts_with('[') && line.ends_with(']') {
      current = &line[1..line.len() - 1];
    } else if current == section
      && let Some((key, value)) = line.split_once('=')
      && key.trim() == name
    {
      return Some(value.trim());
    }
  }
  None
}

fn size_score(candidate: u32, target: u32) -> (u8, u32) {
  if candidate >= target {
    (0, candidate - target)
  } else {
    (1, target - candidate)
  }
}

fn icon_file(dir: &Path, icon: &str) -> Option<PathBuf> {
  ICON_EXTENSIONS
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

fn split_list(value: &str) -> Vec<String> {
  value
    .split(',')
    .map(str::trim)
    .filter(|item| !item.is_empty())
    .map(str::to_owned)
    .collect()
}

pub(super) fn choose_pixmap(pixmaps: &[(i32, i32, Vec<u8>)], icon_size: u32) -> Option<Icon> {
  pixmaps
    .iter()
    .min_by_key(|(width, height, _)| {
      size_score(u32::try_from((*width).min(*height)).unwrap_or(0), icon_size)
    })
    .and_then(|pixmap| icon_from_argb(pixmap.0, pixmap.1, &pixmap.2))
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
  fn render_cache_bound() {
    let mut cache = HashMap::new();
    cache.insert(
      IconRenderKey {
        path: PathBuf::from("old"),
        icon_size: 20,
      },
      CachedIcon {
        modified: None,
        len: 1,
        icon: Icon {
          width: 300,
          height: 300,
          pixels: vec![0; 300 * 300 * 4],
        },
        last_used: 1,
      },
    );
    cache.insert(
      IconRenderKey {
        path: PathBuf::from("new"),
        icon_size: 20,
      },
      CachedIcon {
        modified: None,
        len: 1,
        icon: Icon {
          width: 300,
          height: 300,
          pixels: vec![0; 300 * 300 * 4],
        },
        last_used: 2,
      },
    );

    evict_render_cache(&mut cache);

    assert!(render_cache_bytes(&cache) <= ICON_RENDER_CACHE_MAX_BYTES);
    assert!(cache.contains_key(&IconRenderKey {
      path: PathBuf::from("new"),
      icon_size: 20,
    }));
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
      "[Icon Theme]\nDirectories=24x24/status\n[24x24/status]\nSize=24\n",
    )
    .unwrap();
    fs::write(
      xdg_theme.join("index.theme"),
      "[Icon Theme]\nDirectories=24x24/status\n[24x24/status]\nSize=24\n",
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
        tray_icon_size(1),
      ),
      Some(item_icon)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn larger_icon() {
    let root =
      std::env::temp_dir().join(format!("alpha-bar-tray-large-test-{}", std::process::id()));
    let small = root.join("share/icons/hicolor/16x16/status/input-keyboard.png");
    let large = root.join("share/icons/hicolor/32x32/status/input-keyboard.png");
    fs::create_dir_all(small.parent().unwrap()).unwrap();
    fs::create_dir_all(large.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/hicolor/index.theme"),
      "[Icon Theme]\nDirectories=16x16/status,32x32/status\n[16x16/status]\nSize=16\n[32x32/status]\nSize=32\n",
    )
    .unwrap();
    fs::write(&small, []).unwrap();
    fs::write(&large, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "hicolor",
        tray_icon_size(1)
      ),
      Some(large)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn high_res_icon() {
    let root =
      std::env::temp_dir().join(format!("alpha-bar-tray-hidpi-test-{}", std::process::id()));
    let normal = root.join("share/icons/hicolor/20x20/status/input-keyboard.png");
    let high = root.join("share/icons/hicolor/40x40/status/input-keyboard.png");
    fs::create_dir_all(normal.parent().unwrap()).unwrap();
    fs::create_dir_all(high.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/hicolor/index.theme"),
      "[Icon Theme]\nDirectories=20x20/status,40x40/status\n[20x20/status]\nSize=20\n[40x40/status]\nSize=40\n",
    )
    .unwrap();
    fs::write(&normal, []).unwrap();
    fs::write(&high, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "hicolor",
        tray_icon_size(2)
      ),
      Some(high)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn scaled_icon_dir() {
    let root =
      std::env::temp_dir().join(format!("alpha-bar-tray-scaled-test-{}", std::process::id()));
    let normal = root.join("share/icons/hicolor/20x20/status/input-keyboard.png");
    let scaled = root.join("share/icons/hicolor/20x20@2/status/input-keyboard.png");
    fs::create_dir_all(normal.parent().unwrap()).unwrap();
    fs::create_dir_all(scaled.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/hicolor/index.theme"),
      "[Icon Theme]\nDirectories=20x20/status\nScaledDirectories=20x20@2/status\n[20x20/status]\nSize=20\n[20x20@2/status]\nSize=20\nScale=2\n",
    )
    .unwrap();
    fs::write(&normal, []).unwrap();
    fs::write(&scaled, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "hicolor",
        tray_icon_size(2)
      ),
      Some(scaled)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn prefer_svg() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-svg-prefer-test-{}",
      std::process::id()
    ));
    let png = root.join("share/icons/hicolor/24x24/status/input-keyboard.png");
    let svg = root.join("share/icons/hicolor/24x24/status/input-keyboard.svg");
    fs::create_dir_all(png.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/hicolor/index.theme"),
      "[Icon Theme]\nDirectories=24x24/status\n[24x24/status]\nSize=24\n",
    )
    .unwrap();
    fs::write(&png, []).unwrap();
    fs::write(&svg, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "hicolor",
        tray_icon_size(1)
      ),
      Some(svg)
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
      "[Icon Theme]\nDirectories=scalable/status\n[scalable/status]\nSize=16\n",
    )
    .unwrap();
    fs::write(
      &icon,
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#ff0000"/></svg>"##,
    )
    .unwrap();

    let icon_size = tray_icon_size(2);
    let path = find_icon_path("blueman-action", Some(&root), &[], "hicolor", icon_size).unwrap();
    let rendered = icon_from_path(&path, icon_size).unwrap();

    assert_eq!(path, icon);
    assert_eq!(rendered.width, 80);
    assert_eq!(rendered.height, 40);
    assert!(
      rendered
        .pixels
        .chunks_exact(4)
        .all(|px| px == [0, 0, 255, 255])
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn icon_path_cached() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-path-cache-test-{}",
      std::process::id()
    ));
    let icon = root.join("share/icons/Selected/scalable/status/input-keyboard.svg");
    fs::create_dir_all(icon.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/Selected/index.theme"),
      "[Icon Theme]\nDirectories=scalable/status\n[scalable/status]\nSize=16\n",
    )
    .unwrap();
    fs::write(&icon, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "Selected",
        tray_icon_size(1)
      ),
      Some(icon.clone())
    );
    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "Selected",
        tray_icon_size(1)
      ),
      Some(icon)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn icon_path_cache_invalidates_missing_file() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-path-stale-test-{}",
      std::process::id()
    ));
    let first = root.join("first/icons/Selected/scalable/status/input-keyboard.svg");
    let second = root.join("second/icons/Selected/scalable/status/input-keyboard.svg");
    for icon in [&first, &second] {
      fs::create_dir_all(icon.parent().unwrap()).unwrap();
      fs::write(
        icon
          .parent()
          .unwrap()
          .parent()
          .unwrap()
          .parent()
          .unwrap()
          .join("index.theme"),
        "[Icon Theme]\nDirectories=scalable/status\n[scalable/status]\nSize=16\n",
      )
      .unwrap();
      fs::write(icon, []).unwrap();
    }

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("first"), root.join("second")],
        "Selected",
        tray_icon_size(1),
      ),
      Some(first.clone())
    );
    fs::remove_file(first).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("first"), root.join("second")],
        "Selected",
        tray_icon_size(1),
      ),
      Some(second)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn item_theme_path_not_cached() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-item-path-test-{}",
      std::process::id()
    ));
    let item_theme = root.join("item-theme");
    let icon = item_theme.join("hicolor/scalable/status/input-keyboard.svg");
    fs::create_dir_all(icon.parent().unwrap()).unwrap();
    fs::write(
      item_theme.join("hicolor/index.theme"),
      "[Icon Theme]\nDirectories=scalable/status\n[scalable/status]\nSize=16\n",
    )
    .unwrap();
    fs::write(&icon, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        Some(&item_theme),
        &[],
        "hicolor",
        tray_icon_size(1),
      ),
      Some(icon)
    );
    fs::remove_dir_all(&item_theme).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        Some(&item_theme),
        &[],
        "hicolor",
        tray_icon_size(1),
      ),
      None
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn icon_render_refreshes_changed_file() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-render-cache-test-{}",
      std::process::id()
    ));
    let icon = root.join("hicolor/scalable/status/input-keyboard.svg");
    fs::create_dir_all(icon.parent().unwrap()).unwrap();
    fs::write(
      &icon,
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#00ff00"/></svg>"##,
    )
    .unwrap();

    let icon_size = tray_icon_size(2);
    let rendered = icon_from_path(&icon, icon_size).unwrap();
    fs::write(
      &icon,
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#ff0000"/></svg>"##,
    )
    .unwrap();
    let updated = icon_from_path(&icon, icon_size).unwrap();

    assert_ne!(updated, rendered);
    assert_eq!(updated.width, 80);
    assert_eq!(updated.height, 40);
    assert!(
      updated
        .pixels
        .chunks_exact(4)
        .all(|px| px == [0, 0, 255, 255])
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn icon_render_scales() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-render-scale-test-{}",
      std::process::id()
    ));
    let icon = root.join("hicolor/scalable/status/input-keyboard.svg");
    fs::create_dir_all(icon.parent().unwrap()).unwrap();
    fs::write(
      &icon,
      r##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#00ff00"/></svg>"##,
    )
    .unwrap();

    let normal = icon_from_path(&icon, tray_icon_size(1)).unwrap();
    let scaled = icon_from_path(&icon, tray_icon_size(2)).unwrap();

    assert_eq!(normal.height, 20);
    assert_eq!(scaled.height, 40);

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn pixmap_target_size() {
    let small = vec![0; 22 * 22 * 4];
    let large = vec![0; 64 * 64 * 4];
    let icon = choose_pixmap(&[(22, 22, small), (64, 64, large)], tray_icon_size(1)).unwrap();

    assert_eq!(icon.width, 22);
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
      "[Icon Theme]\nDirectories=symbolic/status\n[symbolic/status]\nSize=16\n",
    )
    .unwrap();
    fs::write(
      root.join("share/icons/Other/index.theme"),
      "[Icon Theme]\nDirectories=symbolic/status\n[symbolic/status]\nSize=16\n",
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
        tray_icon_size(1),
      ),
      Some(selected)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn theme_list_dirs() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-compact-theme-test-{}",
      std::process::id()
    ));
    let selected = root.join("share/icons/Selected/24/panel/input-keyboard.svg");
    fs::create_dir_all(selected.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/Selected/index.theme"),
      "[Icon Theme]\nDirectories=24/panel\nInherits=hicolor\n[24/panel]\nSize=24\n",
    )
    .unwrap();
    fs::write(&selected, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "Selected",
        tray_icon_size(1),
      ),
      Some(selected)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn theme_inherits() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-inherit-theme-test-{}",
      std::process::id()
    ));
    let inherited = root.join("share/icons/Parent/24/panel/input-keyboard.svg");
    fs::create_dir_all(root.join("share/icons/Selected")).unwrap();
    fs::create_dir_all(inherited.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/Selected/index.theme"),
      "[Icon Theme]\nDirectories=\nInherits=Parent\n",
    )
    .unwrap();
    fs::write(
      root.join("share/icons/Parent/index.theme"),
      "[Icon Theme]\nDirectories=24/panel\n[24/panel]\nSize=24\n",
    )
    .unwrap();
    fs::write(&inherited, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "Selected",
        tray_icon_size(1),
      ),
      Some(inherited)
    );

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn theme_index_cached() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-index-cache-test-{}",
      std::process::id()
    ));
    let icon = root.join("share/icons/Selected/24/panel/input-keyboard.svg");
    fs::create_dir_all(icon.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/Selected/index.theme"),
      "[Icon Theme]\nDirectories=24/panel\n[24/panel]\nSize=24\n",
    )
    .unwrap();
    fs::write(&icon, []).unwrap();
    let key = ThemeIndexKey {
      theme: "Selected".into(),
      roots: icon_roots(None, &[root.join("share")]),
    };

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "Selected",
        tray_icon_size(1),
      ),
      Some(icon)
    );
    assert!(theme_index_cache().lock().unwrap().contains_key(&key));

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn theme_uses_size_score() {
    let root = std::env::temp_dir().join(format!(
      "alpha-bar-tray-size-score-test-{}",
      std::process::id()
    ));
    let larger = root.join("share/icons/Selected/24/panel/input-keyboard.svg");
    let closer = root.join("share/icons/Selected/22/panel/input-keyboard.svg");
    fs::create_dir_all(larger.parent().unwrap()).unwrap();
    fs::create_dir_all(closer.parent().unwrap()).unwrap();
    fs::write(
      root.join("share/icons/Selected/index.theme"),
      "[Icon Theme]\nDirectories=24/panel,22/panel\n[24/panel]\nSize=24\n[22/panel]\nSize=22\n",
    )
    .unwrap();
    fs::write(&larger, []).unwrap();
    fs::write(&closer, []).unwrap();

    assert_eq!(
      find_icon_path(
        "input-keyboard",
        None,
        &[root.join("share")],
        "Selected",
        tray_icon_size(1),
      ),
      Some(closer)
    );

    let _ = fs::remove_dir_all(root);
  }
}
