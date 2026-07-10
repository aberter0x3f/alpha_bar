use std::{fs, path::PathBuf};

use alpha_bar::modules::tray::{find_icon_path, icon_from_argb};

#[test]
fn argb_icon_to_bgra() {
  let icon = icon_from_argb(1, 1, &[0xff, 0xff, 0x00, 0x00]).unwrap();
  assert_eq!(icon.pixels, [0x00, 0x00, 0xff, 0xff]);
}

#[test]
fn finds_png_icon_from_item_theme_path_first() {
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
      &[PathBuf::from(root.join("xdg"))],
      "hicolor",
    ),
    Some(item_icon)
  );

  let _ = fs::remove_dir_all(root);
}

#[test]
fn finds_and_renders_svg_icon() {
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
  let rendered = alpha_bar::modules::tray::icon_from_path(&path).unwrap();

  assert_eq!(path, icon);
  assert_eq!(rendered.width, 2);
  assert_eq!(rendered.height, 1);
  assert_eq!(rendered.pixels, [0, 0, 255, 255, 0, 0, 255, 255]);

  let _ = fs::remove_dir_all(root);
}

#[test]
fn follows_configured_theme_and_declared_directories() {
  let root = std::env::temp_dir().join(format!("alpha-bar-tray-theme-test-{}", std::process::id()));
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
      "Selected"
    ),
    Some(selected)
  );

  let _ = fs::remove_dir_all(root);
}
