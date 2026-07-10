use std::{
  fs::{self, File},
  path::{Path, PathBuf},
};

use cairo::{Format, ImageSurface};

use crate::state::Icon;

pub fn icon_from_argb(width: i32, height: i32, argb: &[u8]) -> Option<Icon> {
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

pub fn icon_from_png(path: &Path) -> Option<Icon> {
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

pub fn icon_from_path(path: &Path) -> Option<Icon> {
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

pub fn find_icon_path(
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
