use alpha_bar::net_speed::{format_rate, format_rates, rates_since};

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
fn first_net_speed_sample_only_sets_baseline() {
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
