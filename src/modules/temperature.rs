use std::fs;

use crate::runtime::Update;
use crate::state::{Class, Segment, Side};
use std::{sync::mpsc::Sender, thread, time::Duration};

pub fn current() -> i64 {
  fs::read_to_string("/sys/class/thermal/thermal_zone3/temp")
    .ok()
    .and_then(|s| temp_c(&s))
    .unwrap_or(0)
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn temp() {
    assert_eq!(temp_c("42500\n"), Some(42));
  }
}
