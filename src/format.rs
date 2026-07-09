const CPU_ICONS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const BATTERY_ICONS: [&str; 11] = ["󰂎", "󰁺", "󰁻", "󰁼", "󰁽", "󰁾", "󰁿", "󰂀", "󰂁", "󰂂", "󰁹"];

pub fn cpu_icon(usage: u8) -> &'static str {
  CPU_ICONS[((usage as usize * CPU_ICONS.len()) / 100).min(CPU_ICONS.len() - 1)]
}

pub fn battery_icon(capacity: u8) -> &'static str {
  BATTERY_ICONS[(capacity as usize / 10).min(10)]
}

pub fn state_class(value: u8, warning: u8, critical: u8) -> &'static str {
  if value >= critical {
    "critical"
  } else if value >= warning {
    "warning"
  } else {
    "normal"
  }
}
