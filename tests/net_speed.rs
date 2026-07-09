use alpha_bar::net_speed::{format_rate, format_rates};

#[test]
fn script_format() {
  assert_eq!(format_rate(512 * 1024, 1_000_000), "512 K");
  assert_eq!(format_rate(1536 * 1024, 1_000_000), "1.50 M");
  assert_eq!(
    format_rates(1536 * 1024, 512 * 1024, 1_000_000),
    "↓ 1.50 M ↑ 512 K"
  );
}
