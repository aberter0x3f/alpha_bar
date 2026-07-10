use crate::state::{Segment, Snapshot};

pub fn build_segments(snapshot: &Snapshot) -> Vec<&Segment> {
  let mut segments = Vec::with_capacity(snapshot.workspaces.len() + snapshot.tray.len() + 7);
  segments.extend(snapshot.workspaces.iter());
  if !snapshot.tray.is_empty() {
    segments.extend(snapshot.tray.iter());
  }
  segments.push(&snapshot.network);
  segments.push(&snapshot.net_speed);
  segments.push(&snapshot.audio);
  segments.push(&snapshot.cpu);
  segments.push(&snapshot.memory);
  segments.push(&snapshot.temperature);
  if let Some(battery) = &snapshot.battery {
    segments.push(battery);
  }
  segments.push(&snapshot.clock);
  segments
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::{Class, Icon, Side};

  #[test]
  fn segments() {
    let snapshot = Snapshot {
      workspaces: vec![
        Segment::new(Side::Left, Class::Normal, "1"),
        Segment::new(Side::Left, Class::Active, "2"),
      ],
      tray: vec![Segment::icon(
        Side::Left,
        Class::Normal,
        Icon {
          width: 1,
          height: 1,
          pixels: vec![0, 0, 0, 255],
        },
      )],
      tray_menu: None,
      network: Segment::new(Side::Right, Class::Disconnected, "󰲛 Disconnected"),
      net_speed: Segment::new(Side::Right, Class::Normal, "↓ 0 K ↑ 0 K"),
      audio: Segment::new(Side::Right, Class::Muted, "󰖁 0%"),
      cpu: Segment::new(Side::Right, Class::Warning, "▇ 75%"),
      memory: Segment::new(Side::Right, Class::Critical, "󰍛 91%"),
      temperature: Segment::new(Side::Right, Class::Critical, "󰔄 82"),
      battery: Some(Segment::new(Side::Right, Class::Plugged, " 100%")),
      clock: Segment::new(Side::Right, Class::Normal, "󰥔 09:05"),
    };
    let segments = build_segments(&snapshot);

    assert_eq!(segments[0].side, Side::Left);
    assert_eq!(segments[0].text, "1");
    assert_eq!(segments[1].text, "2");
    assert_eq!(segments[1].class, Class::Active);
    assert!(segments[2].icon.is_some());
    assert_eq!(segments[3].class, Class::Disconnected);
    assert_eq!(segments[5].class, Class::Muted);
    assert_eq!(segments[6].text, "▇ 75%");
    assert_eq!(segments[7].class, Class::Critical);
    assert_eq!(segments[8].class, Class::Critical);
  }

  #[test]
  fn empty_tray() {
    let snapshot = Snapshot {
      workspaces: vec![Segment::new(Side::Left, Class::Active, "1")],
      tray: Vec::new(),
      tray_menu: None,
      network: Segment::new(Side::Right, Class::Disconnected, "󰲛 Disconnected"),
      net_speed: Segment::new(Side::Right, Class::Normal, "↓ 0 K ↑ 0 K"),
      audio: Segment::new(Side::Right, Class::Muted, "󰖁 0%"),
      cpu: Segment::new(Side::Right, Class::Warning, "▇ 75%"),
      memory: Segment::new(Side::Right, Class::Critical, "󰍛 91%"),
      temperature: Segment::new(Side::Right, Class::Critical, "󰔄 82"),
      battery: None,
      clock: Segment::new(Side::Right, Class::Normal, "󰥔 09:05"),
    };
    let segments = build_segments(&snapshot);

    assert_eq!(segments[0].text, "1");
    assert_eq!(segments[1].text, "󰲛 Disconnected");
  }

  #[test]
  fn segments_borrow_snapshot() {
    let snapshot = Snapshot::default();
    let segments = build_segments(&snapshot);

    assert!(std::ptr::eq(segments[0], &snapshot.workspaces[0]));
    assert!(std::ptr::eq(segments[1], &snapshot.network));
  }
}
