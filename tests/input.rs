use alpha_bar::{
  state::{Class, Click, Segment, Side},
  ui::input::{click_at, click_at_with_origin},
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
  assert_eq!(
    click_at_with_origin(200, 50.0, &segments),
    Some((Click::Tray("addr".into()), 34.0))
  );
}

#[test]
fn menu_click_hit() {
  use alpha_bar::state::{TrayMenuEntry, TrayMenuHit, TrayMenuPopup};

  let menu = TrayMenuPopup {
    address: ":1.2/StatusNotifierItem".into(),
    path: "/menu".into(),
    x: 20.0,
    y: 28.0,
    entries: vec![TrayMenuEntry {
      id: 7,
      label: "Exit".into(),
      enabled: true,
      separator: false,
      icon: None,
      children: Vec::new(),
    }],
    open_path: Vec::new(),
  };

  assert_eq!(menu.hit(24.0, 34.0), TrayMenuHit::Activate { id: 7 });
}

#[test]
fn submenu_hit() {
  use alpha_bar::state::{TrayMenuEntry, TrayMenuHit, TrayMenuPopup};

  let mut menu = TrayMenuPopup {
    address: ":1.2/StatusNotifierItem".into(),
    path: "/menu".into(),
    x: 0.0,
    y: 32.0,
    entries: vec![TrayMenuEntry {
      id: 1,
      label: "More".into(),
      enabled: true,
      separator: false,
      icon: None,
      children: vec![TrayMenuEntry {
        id: 2,
        label: "Child".into(),
        enabled: true,
        separator: false,
        icon: None,
        children: Vec::new(),
      }],
    }],
    open_path: Vec::new(),
  };

  assert_eq!(
    menu.hit(4.0, 36.0),
    TrayMenuHit::OpenSubmenu { path: vec![0] }
  );
  menu.open_path = vec![0];
  assert_eq!(
    menu.hit(TrayMenuPopup::column_width(&menu.entries) + 4.0, 36.0),
    TrayMenuHit::Activate { id: 2 }
  );
}
