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
