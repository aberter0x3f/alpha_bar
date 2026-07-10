use std::sync::OnceLock;

use zbus::blocking::Connection;

static SESSION: OnceLock<Connection> = OnceLock::new();

pub(super) fn session() -> zbus::Result<Connection> {
  if let Some(connection) = SESSION.get() {
    return Ok(connection.clone());
  }
  let connection = Connection::session()?;
  let _ = SESSION.set(connection.clone());
  Ok(SESSION.get().unwrap_or(&connection).clone())
}
