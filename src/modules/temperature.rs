use std::fs;

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
  let mut fallback = None;
  for entry in fs::read_dir("/sys/class/thermal").ok()?.flatten() {
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
    if sensor_type.contains("cpu") || sensor_type.contains("CPU") || sensor_type.contains("Package")
    {
      return temp;
    }
    fallback = fallback.or(temp);
  }
  fallback
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn temp() {
    assert_eq!(temp_c("42500\n"), Some(42));
  }
}
