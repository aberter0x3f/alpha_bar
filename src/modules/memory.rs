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

use std::fs;

use crate::{
  runtime::Update,
  state::{Class, Segment, Side},
};
use std::{sync::mpsc::Sender, thread, time::Duration};

const UPDATE_INTERVAL: Duration = Duration::from_secs(1);

pub fn current() -> u8 {
  fs::read_to_string("/proc/meminfo")
    .ok()
    .and_then(|s| used_percent(&s))
    .unwrap_or(0)
}

pub fn segment(memory: u8) -> Segment {
  Segment::new(Side::Right, threshold_class(memory), format!("󰍛 {memory}%"))
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    loop {
      let _ = tx.send(Update::Memory(segment(current())));
      thread::sleep(UPDATE_INTERVAL);
    }
  });
}

fn threshold_class(value: u8) -> Class {
  if value >= 90 {
    Class::Critical
  } else if value >= 70 {
    Class::Warning
  } else {
    Class::Normal
  }
}

fn used_percent(input: &str) -> Option<u8> {
  let total = meminfo_value(input, "MemTotal:")?;
  let available = meminfo_value(input, "MemAvailable:")?;
  let used = total.checked_sub(available)?;
  Some(((used * 100 + total / 2) / total) as u8)
}

fn meminfo_value(input: &str, key: &str) -> Option<u64> {
  input
    .lines()
    .find_map(|line| line.strip_prefix(key))?
    .split_whitespace()
    .next()?
    .parse()
    .ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mem_available_percent() {
    let meminfo = "MemTotal:       1000000 kB\nMemAvailable:    250000 kB\n";
    assert_eq!(used_percent(meminfo), Some(75));
  }
}
