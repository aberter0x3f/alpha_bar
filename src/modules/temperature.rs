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

use std::{fs, path::Path};

use crate::runtime::Update;
use crate::state::{Class, Segment, Side};
use std::{sync::mpsc::Sender, thread, time::Duration};

pub fn current() -> i64 {
  thermal_temp().unwrap_or(0)
}

pub fn segment(temperature: i64) -> Segment {
  let class = if temperature >= 80 {
    Class::Critical
  } else {
    Class::Normal
  };
  Segment::new(Side::Right, class, format!("󰔄 {temperature}"))
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    loop {
      let _ = tx.send(Update::Temperature(segment(current())));
      thread::sleep(Duration::from_secs(5));
    }
  });
}

fn temp_c(raw: &str) -> Option<i64> {
  raw.trim().parse::<i64>().ok().map(|milli| milli / 1000)
}

fn thermal_temp() -> Option<i64> {
  thermal_temp_at(Path::new("/sys/class/thermal"))
}

fn thermal_temp_at(root: &Path) -> Option<i64> {
  let mut cpu = None;
  let mut fallback = None;
  for entry in fs::read_dir(root).ok()?.flatten() {
    let path = entry.path();
    if !path
      .file_name()
      .and_then(|name| name.to_str())
      .is_some_and(|name| name.starts_with("thermal_zone"))
    {
      continue;
    }
    let temp = fs::read_to_string(path.join("temp"))
      .ok()
      .and_then(|raw| temp_c(&raw));
    let sensor_type = fs::read_to_string(path.join("type")).unwrap_or_default();
    if sensor_type.contains("cpu")
      || sensor_type.contains("CPU")
      || sensor_type.contains("Package")
      || sensor_type.contains("package")
    {
      cpu = cpu.max(temp);
    } else {
      fallback = fallback.max(temp);
    }
  }
  cpu.or(fallback)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;

  fn zone(root: &Path, name: &str, sensor_type: &str, temp: &str) {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    fs::write(path.join("type"), sensor_type).unwrap();
    fs::write(path.join("temp"), temp).unwrap();
  }

  #[test]
  fn temp() {
    assert_eq!(temp_c("42500\n"), Some(42));
  }

  #[test]
  fn cpu_max() {
    let root = std::env::temp_dir().join(format!("alpha-bar-temp-test-{}", std::process::id()));
    zone(&root, "thermal_zone0", "cpu_core0", "42000\n");
    zone(&root, "thermal_zone1", "Package id 0", "71000\n");
    zone(&root, "thermal_zone2", "acpitz", "99000\n");

    assert_eq!(thermal_temp_at(&root), Some(71));

    let _ = fs::remove_dir_all(root);
  }
}
