use alpha_bar::power::{battery, temp_c};
use std::fs;

#[test]
fn temp() {
  assert_eq!(temp_c("42500\n"), Some(42));
}

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
fn reads_platform_battery_by_type_not_name() {
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

  let bat = alpha_bar::power::system_battery(&root).unwrap();

  assert_eq!(bat.text, "󰁾 50%");
  assert_eq!(bat.class, "normal");

  let _ = fs::remove_dir_all(root);
}
