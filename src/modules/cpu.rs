use crate::{
  runtime::Update,
  state::{Class, Segment, Side},
};
use std::{fs, sync::mpsc::Sender, thread, time::Duration};

const ICONS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

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
      thread::sleep(Duration::from_secs(5));
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
  let nums = line
    .split_whitespace()
    .skip(1)
    .map(str::parse::<u64>)
    .collect::<Result<Vec<_>, _>>()
    .ok()?;
  let idle = nums.get(3)? + nums.get(4).copied().unwrap_or(0);
  Some((nums.iter().sum(), idle))
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
