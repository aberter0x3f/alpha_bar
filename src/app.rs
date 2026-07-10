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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::{Class, Icon, Side};

  #[test]
  fn segments() {
    let segments = build_segments(&Snapshot {
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
    });

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
    let segments = build_segments(&Snapshot {
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
    });

    assert_eq!(segments[0].text, "1");
    assert_eq!(segments[1].text, "󰲛 Disconnected");
    assert!(!segments.iter().any(|segment| segment.text == "󰂑 N/A"));
  }
}
