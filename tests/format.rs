use alpha_bar::format::{battery_icon, cpu_icon, state_class};

#[test]
fn cpu_icons() {
  assert_eq!(cpu_icon(0), "▁");
  assert_eq!(cpu_icon(13), "▂");
  assert_eq!(cpu_icon(99), "█");
}

#[test]
fn battery_icons() {
  assert_eq!(battery_icon(0), "󰂎");
  assert_eq!(battery_icon(10), "󰁺");
  assert_eq!(battery_icon(100), "󰁹");
}

#[test]
fn state_thresholds() {
  assert_eq!(state_class(69, 70, 90), "normal");
  assert_eq!(state_class(70, 70, 90), "warning");
  assert_eq!(state_class(90, 70, 90), "critical");
}
