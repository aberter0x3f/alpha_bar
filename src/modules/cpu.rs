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

use crate::{
  runtime::Update,
  state::{Class, Segment, Side},
};
use std::{fs, sync::mpsc::Sender, thread, time::Duration};

const ICONS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const UPDATE_INTERVAL: Duration = Duration::from_secs(1);

pub fn current(old: &str, new: &str) -> u8 {
  cpu_usage(old, new).unwrap_or(0)
}

pub fn segment(cpu: u8) -> Segment {
  Segment::new(
    Side::Right,
    threshold_class(cpu),
    format!("{} {}%", icon(cpu), cpu),
  )
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    let mut old = fs::read_to_string("/proc/stat").unwrap_or_default();
    loop {
      thread::sleep(UPDATE_INTERVAL);
      let new = fs::read_to_string("/proc/stat").unwrap_or_default();
      let _ = tx.send(Update::Cpu(segment(current(&old, &new))));
      old = new;
    }
  });
}

fn icon(usage: u8) -> &'static str {
  ICONS[((usage as usize * ICONS.len()) / 100).min(ICONS.len() - 1)]
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

fn cpu_usage(old: &str, new: &str) -> Option<u8> {
  let old = cpu_times(old)?;
  let new = cpu_times(new)?;
  let total = new.0.checked_sub(old.0)?;
  let idle = new.1.checked_sub(old.1)?;
  (total != 0).then(|| (((total - idle) * 100 + total / 2) / total) as u8)
}

fn cpu_times(input: &str) -> Option<(u64, u64)> {
  let line = input.lines().find(|line| line.starts_with("cpu "))?;
  let mut total = 0u64;
  let mut idle = 0u64;
  let mut has_idle = false;
  for (index, field) in line.split_whitespace().skip(1).enumerate() {
    let value = field.parse::<u64>().ok()?;
    total += value;
    if index == 3 || index == 4 {
      idle += value;
      has_idle = true;
    }
  }
  if !has_idle {
    return None;
  }
  Some((total, idle))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn icons() {
    assert_eq!(icon(0), "▁");
    assert_eq!(icon(13), "▂");
    assert_eq!(icon(99), "█");
  }

  #[test]
  fn cpu_delta() {
    let old = "cpu  100 0 100 800 0 0 0 0 0 0";
    let new = "cpu  150 0 150 900 0 0 0 0 0 0";
    assert_eq!(cpu_usage(old, new), Some(50));
  }
}
