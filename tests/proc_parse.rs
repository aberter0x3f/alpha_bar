use alpha_bar::proc_parse::{cpu_usage, mem_usage};

#[test]
fn cpu_delta() {
  let old = "cpu  100 0 100 800 0 0 0 0 0 0";
  let new = "cpu  150 0 150 900 0 0 0 0 0 0";
  assert_eq!(cpu_usage(old, new), Some(50));
}

#[test]
fn mem_available() {
  let meminfo = "MemTotal:       1000000 kB\nMemAvailable:    250000 kB\nSwapTotal:        500000 kB\nSwapFree:         125000 kB\n";
  let mem = mem_usage(meminfo).unwrap();
  assert_eq!(mem.used_percent, 75);
  assert_eq!(mem.used_kib, 750000);
  assert_eq!(mem.swap_used_kib, 375000);
}
