use crate::format::battery_icon;
use std::path::Path;

#[derive(Debug, Eq, PartialEq)]
pub struct Battery {
  pub text: String,
  pub class: &'static str,
}

pub fn temp_c(raw: &str) -> Option<i64> {
  raw.trim().parse::<i64>().ok().map(|milli| milli / 1000)
}

pub fn battery(capacity: &str, status: &str, ac_online: bool) -> Option<Battery> {
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

pub fn system_battery(root: &Path) -> Option<Battery> {
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

fn battery_class(capacity: u8) -> &'static str {
  if capacity <= 15 {
    "critical"
  } else if capacity <= 30 {
    "warning"
  } else {
    "normal"
  }
}
