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

use std::{
  sync::{Arc, Mutex, mpsc::Sender},
  thread,
};

use zbus::{
  MatchRule,
  blocking::{MessageIterator, connection},
  message::Header,
};

use super::{PumpEvent, bus};

pub(super) fn watch_tray(tx: Sender<PumpEvent>) {
  start_watcher(tx.clone());
  let _ = tx.send(PumpEvent::Refresh);
  watch_signals(&tx);
}

#[derive(Clone)]
struct StatusNotifierWatcher {
  items: Arc<Mutex<Vec<String>>>,
  tx: Sender<PumpEvent>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl StatusNotifierWatcher {
  fn register_status_notifier_item(&self, service: String, #[zbus(header)] header: Header<'_>) {
    let address = watcher_address(&service, &header);
    let mut items = self.items.lock().expect("lock tray watcher items");
    if !items.contains(&address) {
      items.push(address);
      drop(items);
      let _ = self.tx.send(PumpEvent::Refresh);
    }
  }

  fn register_status_notifier_host(&self, _service: String) {}

  #[zbus(property)]
  fn registered_status_notifier_items(&self) -> Vec<String> {
    self.items.lock().expect("lock tray watcher items").clone()
  }

  #[zbus(property)]
  fn is_status_notifier_host_registered(&self) -> bool {
    true
  }

  #[zbus(property)]
  fn protocol_version(&self) -> i32 {
    0
  }
}

fn start_watcher(tx: Sender<PumpEvent>) {
  thread::spawn(move || {
    let watcher = StatusNotifierWatcher {
      items: Arc::new(Mutex::new(Vec::new())),
      tx,
    };
    let Ok(_connection) = connection::Builder::session()
      .and_then(|builder| builder.serve_at("/StatusNotifierWatcher", watcher))
      .and_then(|builder| builder.name("org.kde.StatusNotifierWatcher"))
      .and_then(|builder| builder.build())
    else {
      return;
    };
    loop {
      thread::park();
    }
  });
}

fn watcher_address(service: &str, header: &Header<'_>) -> String {
  let sender = header.sender().map(|name| name.as_str()).unwrap_or(service);
  watcher_address_with_sender(service, sender)
}

fn watcher_address_with_sender(service: &str, sender: &str) -> String {
  if service.starts_with('/') {
    format!("{sender}{service}")
  } else {
    service.into()
  }
}

fn watch_signals(tx: &Sender<PumpEvent>) {
  let tx = tx.clone();
  thread::spawn(move || {
    let Ok(connection) = bus::session() else {
      return;
    };
    let rule = MatchRule::builder()
      .msg_type(zbus::message::Type::Signal)
      .build();
    let Ok(messages) = MessageIterator::for_match_rule(rule, &connection, Some(32)) else {
      return;
    };
    for message in messages {
      let Ok(message) = message else {
        continue;
      };
      let Some(interface) = message.header().interface().map(|name| name.to_string()) else {
        continue;
      };
      if watched_interface(&interface) && tx.send(PumpEvent::Refresh).is_err() {
        return;
      }
    }
  });
}

fn watched_interface(interface: &str) -> bool {
  matches!(
    interface,
    "org.kde.StatusNotifierWatcher" | "org.kde.StatusNotifierItem" | "com.canonical.dbusmenu"
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn watcher_address_path() {
    assert_eq!(
      watcher_address_with_sender("/StatusNotifierItem", ":1.2"),
      ":1.2/StatusNotifierItem"
    );
    assert_eq!(watcher_address_with_sender(":1.3", ":1.2"), ":1.3");
  }

  #[test]
  fn signal_filter() {
    assert!(watched_interface("org.kde.StatusNotifierItem"));
    assert!(watched_interface("com.canonical.dbusmenu"));
    assert!(!watched_interface("org.freedesktop.DBus"));
  }
}
