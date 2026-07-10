use alpha_bar::{
  app::build_segments,
  state::{Class, Icon, Segment, Side, Snapshot},
};

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
    network: Segment::new(Side::Right, Class::Disconnected, "󰲛 Disconnected"),
    net_speed: Segment::new(Side::Right, Class::Normal, "↓ 0 K ↑ 0 K"),
    audio: Segment::new(Side::Right, Class::Muted, "󰖁 0%"),
    cpu: Segment::new(Side::Right, Class::Warning, "▇ 75%"),
    memory: Segment::new(Side::Right, Class::Critical, "󰍛 91%"),
    temperature: Segment::new(Side::Right, Class::Critical, "󰔄 82"),
    battery: Segment::new(Side::Right, Class::Plugged, " 100%"),
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
    network: Segment::new(Side::Right, Class::Disconnected, "󰲛 Disconnected"),
    net_speed: Segment::new(Side::Right, Class::Normal, "↓ 0 K ↑ 0 K"),
    audio: Segment::new(Side::Right, Class::Muted, "󰖁 0%"),
    cpu: Segment::new(Side::Right, Class::Warning, "▇ 75%"),
    memory: Segment::new(Side::Right, Class::Critical, "󰍛 91%"),
    temperature: Segment::new(Side::Right, Class::Critical, "󰔄 82"),
    battery: Segment::new(Side::Right, Class::Plugged, " 100%"),
    clock: Segment::new(Side::Right, Class::Normal, "󰥔 09:05"),
  });

  assert_eq!(segments[0].text, "1");
  assert_eq!(segments[1].text, "󰲛 Disconnected");
}
