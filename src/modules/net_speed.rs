use std::{
  fs,
  sync::mpsc::Sender,
  thread,
  time::{Duration, Instant},
};

use crate::{
  modules::network,
  runtime::Update,
  state::{Class, Segment, Side},
};

fn format_rate(diff_bytes: u64, dt_us: u64) -> String {
  let dt_us = dt_us.max(1);
  let kb = diff_bytes.saturating_mul(1_000_000) / dt_us / 1024;
  if kb >= 1024 {
    let mb = kb * 100 / 1024;
    format!("{}.{:02} M", mb / 100, mb % 100)
  } else {
    format!("{kb} K")
  }
}

fn format_rates(rx_diff: u64, tx_diff: u64, dt_us: u64) -> String {
  format!(
    "↓ {} ↑ {}",
    format_rate(rx_diff, dt_us),
    format_rate(tx_diff, dt_us)
  )
}

pub fn rates_since(old: Option<(u64, u64)>, new: (u64, u64), dt_us: u64) -> Option<String> {
  let old = old?;
  Some(format_rates(
    new.0.saturating_sub(old.0),
    new.1.saturating_sub(old.1),
    dt_us,
  ))
}

pub fn segment(text: String) -> Segment {
  Segment::new(Side::Right, Class::Normal, text)
}

pub fn default_segment() -> Segment {
  segment(format_rates(0, 0, 1_000_000))
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    let mut iface = None;
    let mut old = None;
    let mut then = Instant::now();
    loop {
      let route = fs::read_to_string("/proc/net/route").unwrap_or_default();
      let current_iface = network::default_iface(&route).map(str::to_owned);
      if current_iface != iface {
        iface = current_iface;
        old = None;
        then = Instant::now();
      }
      if let Some(name) = iface.as_deref()
        && let Ok(dev) = fs::read_to_string("/proc/net/dev")
        && let Some(new) = network::dev_bytes(&dev, name)
      {
        if let Some(text) = rates_since(old, new, then.elapsed().as_micros() as u64) {
          let _ = tx.send(Update::NetSpeed(segment(text)));
        }
        old = Some(new);
        then = Instant::now();
      }
      thread::sleep(Duration::from_secs(2));
    }
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn script_format() {
    assert_eq!(format_rate(512 * 1024, 1_000_000), "512 K");
    assert_eq!(format_rate(1536 * 1024, 1_000_000), "1.50 M");
    assert_eq!(
      format_rates(1536 * 1024, 512 * 1024, 1_000_000),
      "↓ 1.50 M ↑ 512 K"
    );
  }

  #[test]
  fn first_sample() {
    assert_eq!(rates_since(None, (10_000_000, 20_000_000), 1), None);
    assert_eq!(
      rates_since(
        Some((10_000_000, 20_000_000)),
        (10_512_000, 20_512_000),
        1_000_000
      ),
      Some("↓ 500 K ↑ 500 K".into())
    );
  }
}
