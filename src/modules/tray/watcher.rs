use std::{
  sync::{Arc, Mutex, mpsc::Sender},
  thread,
};

use zbus::{
  MatchRule,
  blocking::{Connection, MessageIterator, connection},
  message::Header,
};

use super::PumpEvent;

pub(super) fn watch_tray(tx: Sender<PumpEvent>) {
  start_watcher(tx.clone());
  let _ = tx.send(PumpEvent::Refresh);
  watch_rule(&tx, "org.kde.StatusNotifierWatcher");
  watch_rule(&tx, "org.kde.StatusNotifierItem");
  watch_rule(&tx, "com.canonical.dbusmenu");
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
    let Ok(messages) = MessageIterator::for_match_rule(rule, &connection, Some(32)) else {
      return;
    };
    for message in messages {
      if message.is_ok() && tx.send(PumpEvent::Refresh).is_err() {
        return;
      }
    }
  });
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
}
