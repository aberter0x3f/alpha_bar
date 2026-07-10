use alpha_bar::{
  state::{Class, Click, Segment, Side},
  ui::input::click_at,
};

#[test]
fn tray_click_hit() {
  let segments = vec![
    Segment::new(Side::Left, Class::Active, "1"),
    Segment::new(Side::Left, Class::Normal, "tray").click(Click::Tray("addr".into())),
  ];
  assert_eq!(
    click_at(200, 50.0, &segments),
    Some(Click::Tray("addr".into()))
  );
  assert_eq!(click_at(200, 10.0, &segments), None);
}
