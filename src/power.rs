use crate::format::battery_icon;

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

fn battery_class(capacity: u8) -> &'static str {
  if capacity <= 15 {
    "critical"
  } else if capacity <= 30 {
    "warning"
  } else {
    "normal"
  }
}
