use alpha_bar::{
  state::{Class, Segment, Side, TrayMenuEntry, TrayMenuPopup},
  ui::render::{render_segments, render_segments_scaled, render_ui_scaled},
};

#[test]
fn render_bar() {
  let pixels = render_segments(64, 32, &[Segment::new(Side::Left, Class::Active, "1")]);

  assert_eq!(pixels.len(), 64 * 32 * 4);
}

#[test]
fn render_scaled() {
  let pixels = render_segments_scaled(80, 32, 2, &[Segment::new(Side::Left, Class::Active, "1")]);

  assert_eq!(pixels.len(), 160 * 64 * 4);
}

#[test]
fn render_menu() {
  let menu = TrayMenuPopup {
    address: ":1.2/StatusNotifierItem".into(),
    path: "/menu".into(),
    x: 0.0,
    y: 0.0,
    entries: vec![TrayMenuEntry {
      id: 1,
      label: "A".into(),
      enabled: true,
      separator: false,
      icon: None,
      children: Vec::new(),
    }],
    open_path: Vec::new(),
  };
  let pixels = render_ui_scaled(200, 64, 1, &[], Some(&menu));

  assert_eq!(pixels.len(), 200 * 64 * 4);
}
