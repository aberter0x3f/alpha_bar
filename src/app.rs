use crate::state::{Segment, Snapshot};

pub fn build_segments(snapshot: &Snapshot) -> Vec<Segment> {
  let mut segments = snapshot.workspaces.clone();
  if !snapshot.tray.is_empty() {
    segments.extend(snapshot.tray.clone());
  }
  let right = [
    snapshot.network.clone(),
    snapshot.net_speed.clone(),
    snapshot.audio.clone(),
    snapshot.cpu.clone(),
    snapshot.memory.clone(),
    snapshot.temperature.clone(),
  ];
  if let Some(battery) = &snapshot.battery {
    segments.extend(right.into_iter().chain([battery.clone()]));
  } else {
    segments.extend(right);
  }
  segments.push(snapshot.clock.clone());
  segments
}
