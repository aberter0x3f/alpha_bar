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
  pub network: Segment,
  pub net_speed: Segment,
  pub audio: Segment,
  pub cpu: Segment,
  pub memory: Segment,
  pub temperature: Segment,
  pub battery: Segment,
  pub clock: Segment,
}
