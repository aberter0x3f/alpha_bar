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
    loop {
      let _ = tx.send(Update::Clock(segment(current())));
      thread::sleep(Duration::from_secs(1));
    }
  });
}

fn current() -> String {
  let now = chrono::Local::now();
  clock_text(now.hour() as u8, now.minute() as u8)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn clock() {
    assert_eq!(clock_text(9, 5), "󰥔 09:05");
  }
}
