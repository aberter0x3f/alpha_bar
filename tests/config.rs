use alpha_bar::config::{BAR, MODULES_LEFT, MODULES_RIGHT, STYLE};

#[test]
fn bar_layout() {
  assert_eq!(BAR.height, 32);
  assert_eq!(BAR.position, "top");
  assert_eq!(BAR.layer, "top");
  assert_eq!(MODULES_LEFT, ["niri/workspaces", "tray"]);
  assert_eq!(
    MODULES_RIGHT,
    [
      "network",
      "net_speed",
      "wireplumber",
      "cpu",
      "memory",
      "temperature",
      "battery",
      "clock"
    ]
  );
}

#[test]
fn style_colors() {
  assert_eq!(STYLE.font, "SarasaMonoSC Nerd Font");
  assert_eq!(STYLE.background, "#3b4252");
  assert_eq!(STYLE.foreground, "#eceff4");
  assert_eq!(STYLE.border_normal, "#4c566a");
  assert_eq!(STYLE.border_active, "#8fbcbb");
  assert_eq!(STYLE.border_warning, "#d08770");
  assert_eq!(STYLE.border_critical, "#bf616a");
}
