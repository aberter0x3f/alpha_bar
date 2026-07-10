use zbus::blocking::{Connection, Proxy};

#[derive(Clone, Default)]
pub(super) struct TrayItem {
  pub(super) icon_theme_path: Option<String>,
  pub(super) icon_name: Option<String>,
  pub(super) icon_pixmap: Vec<(i32, i32, Vec<u8>)>,
}

pub(super) fn tray_items() -> Vec<(String, TrayItem)> {
  full_addresses()
    .into_iter()
    .filter_map(|address| query_item(&address).map(|item| (address, item)))
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

pub(super) fn open_context_menu(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_address(address);
  let connection = Connection::session()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem")?;
  proxy.call_method("ContextMenu", &(x, y))?;
  Ok(())
}

pub(super) fn activate(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_address(address);
  let connection = Connection::session()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem")?;
  proxy.call_method("Activate", &(x, y))?;
  Ok(())
}

pub(super) fn parse_address(address: &str) -> (&str, String) {
  address.split_once('/').map_or(
    (address, "/StatusNotifierItem".into()),
    |(destination, path)| (destination, format!("/{path}")),
  )
}

#[cfg(test)]
mod tests {
  use super::*;

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
  fn tray_order() {
    let addresses = vec![":1.9/Z".into(), ":1.2/A".into(), ":1.10/B".into()];

    assert_eq!(
      sort_addresses(addresses),
      vec![":1.10/B", ":1.2/A", ":1.9/Z"]
    );
  }
}
