/*
  This file is part of Alpha Bar.

  Alpha Bar is free software: you can redistribute it and/or modify it under the terms of the GNU
  General Public License as published by the Free Software Foundation, either version 3 of the
  License, or (at your option) any later version.

  Alpha Bar is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
  even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
  General Public License for more details.

  You should have received a copy of the GNU General Public License along with Alpha Bar. If not,
  see <https://www.gnu.org/licenses/>.
*/

use zbus::Proxy;

use super::bus;

#[derive(Clone, Default)]
pub(super) struct TrayItem {
  pub(super) icon_theme_path: Option<String>,
  pub(super) icon_name: Option<String>,
  pub(super) icon_pixmap: Vec<(i32, i32, Vec<u8>)>,
}

pub(super) fn tray_items() -> Vec<(String, TrayItem)> {
  zbus::block_on(async {
    let mut items = Vec::new();
    for address in full_addresses().await {
      if let Some(item) = query_item(&address).await {
        items.push((address, item));
      }
    }
    items
  })
}

async fn full_addresses() -> Vec<String> {
  let Ok(connection) = bus::session().await else {
    return Vec::new();
  };
  let Ok(proxy) = Proxy::new(
    &connection,
    "org.kde.StatusNotifierWatcher",
    "/StatusNotifierWatcher",
    "org.kde.StatusNotifierWatcher",
  )
  .await
  else {
    return Vec::new();
  };
  sort_addresses(
    proxy
      .get_property::<Vec<String>>("RegisteredStatusNotifierItems")
      .await
      .unwrap_or_default(),
  )
}

fn sort_addresses(mut addresses: Vec<String>) -> Vec<String> {
  addresses.sort();
  addresses
}

async fn query_item(address: &str) -> Option<TrayItem> {
  let (destination, path) = parse_address(address);
  let connection = bus::session().await.ok()?;
  let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem")
    .await
    .ok()?;
  Some(TrayItem {
    icon_theme_path: proxy
      .get_property::<String>("IconThemePath")
      .await
      .ok()
      .filter(|path| !path.is_empty()),
    icon_name: proxy
      .get_property::<String>("IconName")
      .await
      .ok()
      .filter(|name| !name.is_empty()),
    icon_pixmap: proxy
      .get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap")
      .await
      .unwrap_or_default(),
  })
}

pub(super) fn open_context_menu(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  zbus::block_on(async {
    let (destination, path) = parse_address(address);
    let connection = bus::session().await?;
    let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").await?;
    proxy.call_method("ContextMenu", &(x, y)).await?;
    Ok(())
  })
}

pub(super) fn activate(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  zbus::block_on(async {
    let (destination, path) = parse_address(address);
    let connection = bus::session().await?;
    let proxy = Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").await?;
    proxy.call_method("Activate", &(x, y)).await?;
    Ok(())
  })
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
