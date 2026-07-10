#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
  Left,
  Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Class {
  Normal,
  Active,
  Warning,
  Critical,
  Muted,
  Disconnected,
  Charging,
  Plugged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Icon {
  pub width: usize,
  pub height: usize,
  pub pixels: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Click {
  Tray(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayMenuEntry {
  pub id: i32,
  pub label: String,
  pub enabled: bool,
  pub separator: bool,
  pub icon: Option<Icon>,
  pub children: Vec<TrayMenuEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrayMenuHit {
  Activate { id: i32 },
  OpenSubmenu { path: Vec<usize> },
  Outside,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrayMenuPopup {
  pub address: String,
  pub path: String,
  pub x: f64,
  pub y: f64,
  pub entries: Vec<TrayMenuEntry>,
  pub open_path: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Segment {
  pub side: Side,
  pub class: Class,
  pub text: String,
  pub icon: Option<Icon>,
  pub click: Option<Click>,
}

impl Segment {
  pub fn new(side: Side, class: Class, text: impl Into<String>) -> Self {
    Self {
      side,
      class,
      text: text.into(),
      icon: None,
      click: None,
    }
  }

  pub fn icon(side: Side, class: Class, icon: Icon) -> Self {
    Self {
      side,
      class,
      text: String::new(),
      icon: Some(icon),
      click: None,
    }
  }

  pub fn click(mut self, click: Click) -> Self {
    self.click = Some(click);
    self
  }
}

pub struct Snapshot {
  pub workspaces: Vec<Segment>,
  pub tray: Vec<Segment>,
  pub tray_menu: Option<TrayMenuPopup>,
  pub network: Segment,
  pub net_speed: Segment,
  pub audio: Segment,
  pub cpu: Segment,
  pub memory: Segment,
  pub temperature: Segment,
  pub battery: Option<Segment>,
  pub clock: Segment,
}
