use alpha_bar::power::{battery, temp_c};

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
