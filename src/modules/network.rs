use std::{
  fs,
  sync::mpsc::Sender,
  thread,
  time::{Duration, Instant},
};

use crate::{
  runtime::Update,
  state::{Class, Segment, Side},
};

pub fn dev_bytes(input: &str, iface: &str) -> Option<(u64, u64)> {
  let prefix = format!("{iface}:");
  let line = input
    .lines()
    .find(|line| line.trim_start().starts_with(&prefix))?;
  let data = line.split_once(':')?.1;
  let mut cols = data.split_whitespace();
  let rx = cols.next()?.parse::<u64>().ok()?;
  let tx = cols.nth(7)?.parse::<u64>().ok()?;
  Some((rx, tx))
}

pub fn segment(text: String) -> Segment {
  let class = if text.contains("Disconnected") {
    Class::Disconnected
  } else {
    Class::Normal
  };
  Segment::new(Side::Right, class, text)
}

pub fn speed_segment(text: String) -> Segment {
  Segment::new(Side::Right, Class::Normal, text)
}

pub fn default_speed_segment() -> Segment {
  speed_segment(format_rates(0, 0, 1_000_000))
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    let mut iface = None;
    let mut old = None;
    let mut then = Instant::now();
    loop {
      let route = fs::read_to_string("/proc/net/route").unwrap_or_default();
      let current_iface = default_iface(&route).map(str::to_owned);
      let operstate = current_iface
        .as_deref()
        .and_then(|iface| fs::read_to_string(format!("/sys/class/net/{iface}/operstate")).ok())
        .unwrap_or_default();
      let _ = tx.send(Update::Network(segment(text(
        current_iface.as_deref(),
        &operstate,
      ))));
      if current_iface != iface {
        iface = current_iface;
        old = None;
        then = Instant::now();
      }
      if let Some(name) = iface.as_deref()
        && let Ok(dev) = fs::read_to_string("/proc/net/dev")
        && let Some(new) = dev_bytes(&dev, name)
      {
        if let Some(text) = rates_since(old, new, then.elapsed().as_micros() as u64) {
          let _ = tx.send(Update::NetSpeed(speed_segment(text)));
        }
        old = Some(new);
        then = Instant::now();
      }
      thread::sleep(Duration::from_secs(2));
    }
  });
}

pub fn text(iface: Option<&str>, operstate: &str) -> String {
  match (iface, operstate.trim()) {
    (None, _) => "󰲛 Disconnected".into(),
    (Some(iface), "up") if iface.starts_with('w') => {
      wifi_text(iface, wireless_ssid(iface).as_deref())
    }
    (Some(iface), "up") => format!("󰈀 {iface}"),
    (Some(iface), _) => format!("{iface} (No IP)"),
  }
}

pub fn wifi_text(iface: &str, ssid: Option<&str>) -> String {
  format!("󰤨 {}", ssid.filter(|s| !s.is_empty()).unwrap_or(iface))
}

pub fn default_iface(route: &str) -> Option<&str> {
  route.lines().skip(1).find_map(|line| {
    let mut cols = line.split_whitespace();
    let iface = cols.next()?;
    let destination = cols.next()?;
    let _gateway = cols.next()?;
    let flags = u16::from_str_radix(cols.next()?, 16).ok()?;
    (destination == "00000000" && flags & 0x2 != 0).then_some(iface)
  })
}

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

fn rates_since(old: Option<(u64, u64)>, new: (u64, u64), dt_us: u64) -> Option<String> {
  let old = old?;
  Some(format_rates(
    new.0.saturating_sub(old.0),
    new.1.saturating_sub(old.1),
    dt_us,
  ))
}

fn wireless_ssid(iface: &str) -> Option<String> {
  const SIOCGIWESSID: libc::c_ulong = 0x8B1B;
  #[repr(C)]
  struct IwPoint {
    pointer: *mut libc::c_void,
    length: u16,
    flags: u16,
  }
  #[repr(C)]
  union IwreqData {
    essid: std::mem::ManuallyDrop<IwPoint>,
  }
  #[repr(C)]
  struct Iwreq {
    name: [libc::c_char; libc::IFNAMSIZ],
    data: IwreqData,
  }

  let mut essid = [0u8; 33];
  let mut req = Iwreq {
    name: [0; libc::IFNAMSIZ],
    data: IwreqData {
      essid: std::mem::ManuallyDrop::new(IwPoint {
        pointer: essid.as_mut_ptr().cast(),
        length: 32,
        flags: 0,
      }),
    },
  };
  for (dst, src) in req.name.iter_mut().zip(iface.bytes()) {
    *dst = src as libc::c_char;
  }
  let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
  if fd < 0 {
    return None;
  }
  let ok = unsafe { libc::ioctl(fd, SIOCGIWESSID, &mut req) } == 0;
  let _ = unsafe { libc::close(fd) };
  if !ok {
    return None;
  }
  let len = unsafe { req.data.essid.length as usize }.min(essid.len());
  String::from_utf8(
    essid[..len]
      .iter()
      .copied()
      .take_while(|b| *b != 0)
      .collect(),
  )
  .ok()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn proc_net_dev() {
    let dev = "Inter-| Receive | Transmit\n  face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n  wlan0: 1000 0 0 0 0 0 0 0 9000 0 0 0 0 0 0 0\n";
    assert_eq!(dev_bytes(dev, "wlan0"), Some((1000, 9000)));
  }

  #[test]
  fn default_route() {
    let route = "Iface\tDestination\tGateway\tFlags\nlo\t00000000\t00000000\t0001\nwlan0\t00000000\t0102A8C0\t0003\n";
    assert_eq!(default_iface(route), Some("wlan0"));
  }

  #[test]
  fn network_states() {
    assert_eq!(text(None, "up"), "󰲛 Disconnected");
    assert_eq!(text(Some("enp4s0"), "up"), "󰈀 enp4s0");
    assert_eq!(text(Some("wlan0"), "down"), "wlan0 (No IP)");
  }

  #[test]
  fn wifi_ssid() {
    assert_eq!(wifi_text("wlan0", Some("CafeNet")), "󰤨 CafeNet");
    assert_eq!(wifi_text("wlan0", None), "󰤨 wlan0");
  }

  #[test]
  fn speed_format() {
    assert_eq!(format_rate(512 * 1024, 1_000_000), "512 K");
    assert_eq!(format_rate(1536 * 1024, 1_000_000), "1.50 M");
    assert_eq!(
      format_rates(1536 * 1024, 512 * 1024, 1_000_000),
      "↓ 1.50 M ↑ 512 K"
    );
  }

  #[test]
  fn speed_first_sample() {
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
