pub struct BarConfig {
  pub height: u32,
  pub position: &'static str,
  pub layer: &'static str,
}

pub struct StyleConfig {
  pub font: &'static str,
  pub background: &'static str,
  pub foreground: &'static str,
  pub border_normal: &'static str,
  pub border_active: &'static str,
  pub border_warning: &'static str,
  pub border_critical: &'static str,
}

pub const BAR: BarConfig = BarConfig {
  height: 32,
  position: "top",
  layer: "top",
};

pub const STYLE: StyleConfig = StyleConfig {
  font: "SarasaMonoSC Nerd Font",
  background: "#3b4252",
  foreground: "#eceff4",
  border_normal: "#4c566a",
  border_active: "#8fbcbb",
  border_warning: "#d08770",
  border_critical: "#bf616a",
};
