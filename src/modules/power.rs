use crate::runtime::Update;
use crate::state::{Class, Segment, Side};
use std::path::Path;
use std::{sync::mpsc::Sender, thread, time::Duration};

const ICONS: [&str; 11] = ["󰂎", "󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹"];

#[derive(Debug, Eq, PartialEq)]
struct Battery {
  text: String,
  class: &'static str,
}

fn battery(capacity: &str, status: &str, ac_online: bool) -> Option<Battery> {
  let capacity = capacity.trim().parse::<u8>().ok()?;
  let status = status.trim();
  if status == "Charging" {
    Some(Battery {
      text: format!("󰂄 {capacity}%"),
      class: "charging",
    })
  } else if ac_online || status == "Full" || status == "Not charging" {
    Some(Battery {
      text: format!(" {capacity}%"),
      class: "plugged",
    })
  } else {
    Some(Battery {
      text: format!("{} {capacity}%", battery_icon(capacity)),
      class: battery_class(capacity),
    })
  }
}

fn system_battery(root: &Path) -> Option<Battery> {
  std::fs::read_dir(root).ok()?.flatten().find_map(|entry| {
    let path = entry.path();
    if std::fs::read_to_string(path.join("type")).ok()?.trim() != "Battery" {
      return None;
    }
    if std::fs::read_to_string(path.join("scope"))
      .ok()
      .is_some_and(|scope| scope.trim().eq_ignore_ascii_case("device"))
    {
      return None;
    }
    let capacity = std::fs::read_to_string(path.join("capacity")).ok()?;
    let status = std::fs::read_to_string(path.join("status")).unwrap_or_else(|_| "Unknown".into());
    battery(&capacity, &status, false)
  })
}

pub fn segment(text: String, class: &str) -> Segment {
  Segment::new(Side::Right, class_from_battery(class), text)
}

pub fn battery_segment(root: &Path) -> Option<Segment> {
  system_battery(root).map(|battery| segment(battery.text, battery.class))
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    loop {
      let _ = tx.send(Update::Battery(battery_segment(Path::new(
        "/sys/class/power_supply",
      ))));
      thread::sleep(Duration::from_secs(5));
    }
  });
}

fn class_from_battery(class: &str) -> Class {
  match class {
    "warning" => Class::Warning,
    "critical" => Class::Critical,
    "charging" => Class::Charging,
    "plugged" => Class::Plugged,
    _ => Class::Normal,
  }
}

fn battery_class(capacity: u8) -> &'static str {
  if capacity <= 15 {
    "critical"
  } else if capacity <= 30 {
    "warning"
  } else {
    "normal"
  }
}

fn battery_icon(capacity: u8) -> &'static str {
  ICONS[(capacity as usize / 10).min(10)]
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs;

  #[test]
  fn battery_state() {
    let bat = battery("87\n", "Discharging\n", false).unwrap();
    assert_eq!(bat.text, "󰂁 87%");
    assert_eq!(bat.class, "normal");

    let bat = battery("14\n", "Discharging\n", false).unwrap();
    assert_eq!(bat.class, "critical");

    let bat = battery("42\n", "Charging\n", false).unwrap();
    assert_eq!(bat.text, "󰂄 42%");
    assert_eq!(bat.class, "charging");

    let bat = battery("99\n", "Full\n", true).unwrap();
    assert_eq!(bat.text, " 99%");
    assert_eq!(bat.class, "plugged");
  }

  #[test]
  fn battery_icons() {
    assert_eq!(battery_icon(0), "󰂎");
    assert_eq!(battery_icon(10), "󰁺");
    assert_eq!(battery_icon(100), "󰁹");
  }

  #[test]
  fn mac_battery() {
    let root = std::env::temp_dir().join(format!("alpha-bar-power-{}", std::process::id()));
    let battery_dir = root.join("macsmc-battery");
    let usb_dir = root.join("tps6598x-source-psy-0-003a");
    fs::create_dir_all(&battery_dir).unwrap();
    fs::create_dir_all(&usb_dir).unwrap();
    fs::write(battery_dir.join("type"), "Battery\n").unwrap();
    fs::write(battery_dir.join("capacity"), "50\n").unwrap();
    fs::write(battery_dir.join("status"), "Discharging\n").unwrap();
    fs::write(usb_dir.join("type"), "USB\n").unwrap();
    fs::write(usb_dir.join("capacity"), "100\n").unwrap();
    fs::write(usb_dir.join("status"), "Charging\n").unwrap();

    let bat = system_battery(&root).unwrap();

    assert_eq!(bat.text, "󰁾 50%");
    assert_eq!(bat.class, "normal");

    let _ = fs::remove_dir_all(root);
  }

  #[test]
  fn no_battery() {
    let root = std::env::temp_dir().join(format!("alpha-bar-no-power-{}", std::process::id()));
    let mains = root.join("AC");
    fs::create_dir_all(&mains).unwrap();
    fs::write(mains.join("type"), "Mains\n").unwrap();

    assert_eq!(battery_segment(&root), None);

    let _ = fs::remove_dir_all(root);
  }
}
