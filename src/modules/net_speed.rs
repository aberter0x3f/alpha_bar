pub fn format_rate(diff_bytes: u64, dt_us: u64) -> String {
  let dt_us = dt_us.max(1);
  let kb = diff_bytes.saturating_mul(1_000_000) / dt_us / 1024;
  if kb >= 1024 {
    let mb = kb * 100 / 1024;
    format!("{}.{:02} M", mb / 100, mb % 100)
  } else {
    format!("{kb} K")
  }
}

pub fn format_rates(rx_diff: u64, tx_diff: u64, dt_us: u64) -> String {
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
