use crate::{
  format::{cpu_icon, state_class},
  state::{Class, Segment, Side},
};

pub struct Snapshot {
  pub workspaces: Vec<Segment>,
  pub tray: Vec<Segment>,
  pub network: String,
  pub net_speed: String,
  pub audio: String,
  pub cpu: u8,
  pub memory: u8,
  pub temperature: i64,
  pub battery: String,
  pub battery_class: Class,
  pub clock: String,
}

pub fn build_segments(snapshot: &Snapshot) -> Vec<Segment> {
  let mut segments = snapshot.workspaces.clone();
  segments.extend([
    Segment::new(
      Side::Right,
      network_class(&snapshot.network),
      snapshot.network.clone(),
    ),
    Segment::new(Side::Right, Class::Normal, snapshot.net_speed.clone()),
    Segment::new(
      Side::Right,
      audio_class(&snapshot.audio),
      snapshot.audio.clone(),
    ),
    Segment::new(
      Side::Right,
      class_from_threshold(snapshot.cpu),
      format!("{} {}%", cpu_icon(snapshot.cpu), snapshot.cpu),
    ),
    Segment::new(
      Side::Right,
      class_from_threshold(snapshot.memory),
      format!("󰍛 {}%", snapshot.memory),
    ),
    Segment::new(
      Side::Right,
      temp_class(snapshot.temperature),
      format!("󰔄 {}", snapshot.temperature),
    ),
    Segment::new(
      Side::Right,
      snapshot.battery_class,
      snapshot.battery.clone(),
    ),
    Segment::new(Side::Right, Class::Normal, snapshot.clock.clone()),
  ]);
  if !snapshot.tray.is_empty() {
    let index = segments
      .iter()
      .position(|segment| segment.side == Side::Right)
      .unwrap_or(segments.len());
    segments.splice(index..index, snapshot.tray.clone());
  }
  segments
}

fn class_from_threshold(value: u8) -> Class {
  match state_class(value, 70, 90) {
    "warning" => Class::Warning,
    "critical" => Class::Critical,
    _ => Class::Normal,
  }
}

fn temp_class(value: i64) -> Class {
  if value >= 80 {
    Class::Critical
  } else {
    Class::Normal
  }
}

fn network_class(text: &str) -> Class {
  if text.contains("Disconnected") {
    Class::Disconnected
  } else {
    Class::Normal
  }
}

fn audio_class(text: &str) -> Class {
  if text.starts_with('󰖁') {
    Class::Muted
  } else {
    Class::Normal
  }
}
