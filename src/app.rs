use crate::state::{Segment, Side, Snapshot};

pub fn build_segments(snapshot: &Snapshot) -> Vec<Segment> {
  let mut segments = snapshot.workspaces.clone();
  if !snapshot.tray.is_empty() {
    segments.extend(snapshot.tray.clone());
  }
  segments.extend([
    snapshot.network.clone(),
    snapshot.net_speed.clone(),
    snapshot.audio.clone(),
    snapshot.cpu.clone(),
    snapshot.memory.clone(),
    snapshot.temperature.clone(),
    snapshot.battery.clone(),
    snapshot.clock.clone(),
  ]);
  segments.sort_by_key(|segment| match segment.side {
    Side::Left => 0,
    Side::Right => 1,
  });
  segments
}
