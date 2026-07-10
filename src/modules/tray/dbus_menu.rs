use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  time::{SystemTime, UNIX_EPOCH},
};

use zbus::{
  blocking::Proxy,
  zvariant::{Array, OwnedObjectPath, OwnedValue, Structure, Value},
};

use crate::state::{Icon, TrayMenuEntry, TrayMenuPopup};

use super::{
  bus,
  icons::{current_icon_theme, icon_from_name, icon_from_png_data, xdg_data_roots},
  sni::parse_address,
};

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

pub(super) fn activate_menu_item(address: &str, path: &str, id: i32) -> zbus::Result<()> {
  let (destination, _) = parse_address(address);
  let connection = bus::session()?;
  let proxy = Proxy::new(&connection, destination, path, "com.canonical.dbusmenu")?;
  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_secs() as u32);
  proxy.call_method("Event", &(id, "clicked", Value::I32(0), timestamp))?;
  Ok(())
}

pub(super) fn menu_address(address: &str) -> Option<(String, Option<String>)> {
  let (destination, path) = parse_address(address);
  let connection = bus::session().ok()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").ok()?;
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

pub(super) fn refresh_tray_menu(
  address: String,
  path: String,
  item_theme_path: Option<String>,
  x: i32,
  icon_size: u32,
) -> zbus::Result<TrayMenuPopup> {
  let (destination, _) = parse_address(&address);
  let connection = bus::session()?;
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
    icon_size,
  );
  Ok(TrayMenuPopup {
    address,
    path,
    x: x.max(0) as f64,
    y: 0.0,
    entries,
    open_path: Vec::new(),
  })
}

pub(super) fn pending_menu(address: String, path: String, x: i32) -> TrayMenuPopup {
  TrayMenuPopup {
    address,
    path,
    x: x.max(0) as f64,
    y: 0.0,
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
  icon_size: u32,
) -> Vec<TrayMenuEntry> {
  items
    .iter()
    .filter_map(|item| {
      menu_entry(
        item,
        item_theme_path,
        xdg_data_roots,
        icon_theme,
        icon_size,
        true,
      )
    })
    .collect()
}

fn menu_entry(
  item: &MenuItem,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  icon_theme: &str,
  icon_size: u32,
  resolve_icon: bool,
) -> Option<TrayMenuEntry> {
  if !item.visible {
    return None;
  }
  let separator = item.menu_type == MenuType::Separator;
  let children = item
    .submenu
    .iter()
    .filter_map(|child| {
      menu_entry(
        child,
        item_theme_path,
        xdg_data_roots,
        icon_theme,
        icon_size,
        false,
      )
    })
    .collect();
  Some(TrayMenuEntry {
    id: item.id,
    label: item.label.clone().unwrap_or_default(),
    enabled: item.enabled,
    separator,
    icon: resolve_icon
      .then(|| menu_item_icon(item, item_theme_path, xdg_data_roots, icon_theme, icon_size))
      .flatten(),
    children,
  })
}

fn menu_item_icon(
  item: &MenuItem,
  item_theme_path: Option<&Path>,
  xdg_data_roots: &[PathBuf],
  icon_theme: &str,
  icon_size: u32,
) -> Option<Icon> {
  item
    .icon_name
    .as_deref()
    .and_then(|name| icon_from_name(name, item_theme_path, xdg_data_roots, icon_theme, icon_size))
    .or_else(|| item.icon_data.as_deref().and_then(icon_from_png_data))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::modules::tray::icons::tray_icon_size;
  use std::io::Cursor;

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

    let entries = menu_entries(&menu, None, &[], "hicolor", tray_icon_size(1));

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label, "Parent");
    assert_eq!(entries[0].children[0].label, "Child");
  }

  #[test]
  fn submenu_icon_lazy() {
    let png = png_icon();
    let menu = vec![MenuItem {
      id: 1,
      label: Some("Parent".into()),
      enabled: true,
      visible: true,
      icon_data: Some(png.clone()),
      submenu: vec![MenuItem {
        id: 2,
        label: Some("Child".into()),
        enabled: true,
        visible: true,
        icon_data: Some(png),
        ..Default::default()
      }],
      ..Default::default()
    }];

    let entries = menu_entries(&menu, None, &[], "hicolor", tray_icon_size(1));

    assert!(entries[0].icon.is_some());
    assert!(entries[0].children[0].icon.is_none());
  }

  fn png_icon() -> Vec<u8> {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let mut data = Cursor::new(Vec::new());
    surface.write_to_png(&mut data).unwrap();
    data.into_inner()
  }

  #[test]
  fn loading_menu() {
    let menu = pending_menu(":1.2/StatusNotifierItem".into(), "/menu".into(), 12);

    assert_eq!(menu.entries.len(), 1);
    assert_eq!(menu.entries[0].label, "Loading...");
    assert!(!menu.entries[0].enabled);
  }
}
