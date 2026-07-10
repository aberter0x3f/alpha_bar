pub fn dev_bytes(input: &str, iface: &str) -> Option<(u64, u64)> {
  let prefix = format!("{iface}:");
  let line = input
    .lines()
    .find(|line| line.trim_start().starts_with(&prefix))?;
  let data = line.split_once(':')?.1;
  let nums = data
    .split_whitespace()
    .map(str::parse::<u64>)
    .collect::<Result<Vec<_>, _>>()
    .ok()?;
  Some((*nums.first()?, *nums.get(8)?))
}

pub fn rate(old: (u64, u64), new: (u64, u64), dt_us: u64) -> (u64, u64) {
  let dt_us = dt_us.max(1);
  (
    new.0.saturating_sub(old.0) * 1_000_000 / dt_us,
    new.1.saturating_sub(old.1) * 1_000_000 / dt_us,
  )
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
