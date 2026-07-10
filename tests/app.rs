use alpha_bar::{
  app::{Snapshot, build_segments},
  state::{Class, Icon, Segment, Side},
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
    network: "󰲛 Disconnected".into(),
    net_speed: "↓ 0 K ↑ 0 K".into(),
    audio: "󰖁 0%".into(),
    cpu: 75,
    memory: 91,
    temperature: 82,
    battery: " 100%".into(),
    battery_class: Class::Plugged,
    clock: "󰥔 09:05".into(),
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
fn empty_tray_is_not_rendered() {
  let segments = build_segments(&Snapshot {
    workspaces: vec![Segment::new(Side::Left, Class::Active, "1")],
    tray: Vec::new(),
    network: "󰲛 Disconnected".into(),
    net_speed: "↓ 0 K ↑ 0 K".into(),
    audio: "󰖁 0%".into(),
    cpu: 75,
    memory: 91,
    temperature: 82,
    battery: " 100%".into(),
    battery_class: Class::Plugged,
    clock: "󰥔 09:05".into(),
  });

  assert_eq!(segments[0].text, "1");
  assert_eq!(segments[1].text, "󰲛 Disconnected");
}
