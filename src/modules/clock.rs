use crate::runtime::Update;
use crate::state::{Class, Segment, Side};
use std::{
  mem::MaybeUninit,
  sync::mpsc::Sender,
  thread,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

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
      let Some((hour, minute, second, nanosecond)) = local_time() else {
        thread::sleep(Duration::from_secs(1));
        continue;
      };
      let text = clock_text(hour, minute);
      if text != last {
        last = text.clone();
        let _ = tx.send(Update::Clock(segment(text)));
      }
      thread::sleep(delay_to_next_minute(second, nanosecond));
    }
  });
}

fn local_time() -> Option<(u8, u8, u32, u32)> {
  let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
  let seconds = now.as_secs() as libc::time_t;
  let mut time = MaybeUninit::<libc::tm>::uninit();
  let time = unsafe {
    if libc::localtime_r(&seconds, time.as_mut_ptr()).is_null() {
      return None;
    }
    time.assume_init()
  };
  Some((
    time.tm_hour as u8,
    time.tm_min as u8,
    time.tm_sec as u32,
    now.subsec_nanos(),
  ))
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
