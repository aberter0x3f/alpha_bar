use crate::runtime::Update;
use crate::state::{Class, Segment, Side};
use chrono::Timelike;
use std::{sync::mpsc::Sender, thread, time::Duration};

pub fn clock_text(hour: u8, minute: u8) -> String {
  format!("󰥔 {hour:02}:{minute:02}")
}

pub fn segment(text: String) -> Segment {
  Segment::new(Side::Right, Class::Normal, text)
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    let mut last = String::new();
    loop {
      let now = chrono::Local::now();
      let text = clock_text(now.hour() as u8, now.minute() as u8);
      if text != last {
        last = text.clone();
        let _ = tx.send(Update::Clock(segment(text)));
      }
      thread::sleep(delay_to_next_minute(now.second(), now.nanosecond()));
    }
  });
}

fn delay_to_next_minute(second: u32, nanosecond: u32) -> Duration {
  let remaining = 60_000_000_000u64
    .saturating_sub(second as u64 * 1_000_000_000 + nanosecond as u64)
    .max(1_000_000);
  Duration::from_nanos(remaining)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clock() {
    assert_eq!(clock_text(9, 5), "󰥔 09:05");
  }

  #[test]
  fn next_minute_delay() {
    assert_eq!(delay_to_next_minute(12, 0).as_secs(), 48);
    assert_eq!(delay_to_next_minute(59, 0).as_secs(), 1);
  }
}
