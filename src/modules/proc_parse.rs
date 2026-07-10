#[derive(Debug, Eq, PartialEq)]
pub struct MemUsage {
  pub used_percent: u8,
  pub used_kib: u64,
  pub total_kib: u64,
  pub swap_used_kib: u64,
  pub swap_total_kib: u64,
}

pub fn cpu_usage(old: &str, new: &str) -> Option<u8> {
  let old = cpu_times(old)?;
  let new = cpu_times(new)?;
  let total = new.0.checked_sub(old.0)?;
  let idle = new.1.checked_sub(old.1)?;
  (total != 0).then(|| (((total - idle) * 100 + total / 2) / total) as u8)
}

fn cpu_times(input: &str) -> Option<(u64, u64)> {
  let line = input.lines().find(|line| line.starts_with("cpu "))?;
  let nums = line
    .split_whitespace()
    .skip(1)
    .map(str::parse::<u64>)
    .collect::<Result<Vec<_>, _>>()
    .ok()?;
  let idle = nums.get(3)? + nums.get(4).copied().unwrap_or(0);
  Some((nums.iter().sum(), idle))
}

pub fn mem_usage(input: &str) -> Option<MemUsage> {
  let total = meminfo_value(input, "MemTotal:")?;
  let available = meminfo_value(input, "MemAvailable:")?;
  let swap_total = meminfo_value(input, "SwapTotal:").unwrap_or(0);
  let swap_free = meminfo_value(input, "SwapFree:").unwrap_or(0);
  let used = total.checked_sub(available)?;
  Some(MemUsage {
    used_percent: ((used * 100 + total / 2) / total) as u8,
    used_kib: used,
    total_kib: total,
    swap_used_kib: swap_total.saturating_sub(swap_free),
    swap_total_kib: swap_total,
  })
}

fn meminfo_value(input: &str, key: &str) -> Option<u64> {
  input
    .lines()
    .find_map(|line| line.strip_prefix(key))?
    .split_whitespace()
    .next()?
    .parse()
    .ok()
}
