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

impl TrayMenuPopup {
  const ROW_HEIGHT: f64 = 24.0;
  const ICON_WIDTH: f64 = 28.0;
  const ARROW_WIDTH: f64 = 18.0;
  const MIN_WIDTH: f64 = 160.0;

  pub fn columns(&self) -> Vec<&[TrayMenuEntry]> {
    let mut columns = vec![self.entries.as_slice()];
    let mut entries = self.entries.as_slice();
    for &index in &self.open_path {
      let Some(entry) = entries.get(index) else {
        break;
      };
      if entry.children.is_empty() {
        break;
      }
      entries = entry.children.as_slice();
      columns.push(entries);
    }
    columns
  }

  pub fn column_width(entries: &[TrayMenuEntry]) -> f64 {
    entries
      .iter()
      .map(|entry| {
        if entry.separator {
          Self::MIN_WIDTH
        } else {
          entry.label.chars().count() as f64 * 10.0 + Self::ICON_WIDTH + Self::ARROW_WIDTH + 16.0
        }
      })
      .fold(Self::MIN_WIDTH, f64::max)
  }

  pub fn width(&self) -> f64 {
    self
      .columns()
      .iter()
      .map(|entries| Self::column_width(entries))
      .sum()
  }

  pub fn height(&self) -> f64 {
    self
      .columns()
      .iter()
      .map(|entries| entries.len() as f64 * Self::ROW_HEIGHT)
      .fold(0.0, f64::max)
  }

  pub fn hit(&self, x: f64, y: f64) -> TrayMenuHit {
    if x < self.x || y < self.y || x >= self.x + self.width() || y >= self.y + self.height() {
      return TrayMenuHit::Outside;
    }
    let mut column_x = self.x;
    let mut path = Vec::new();
    for entries in self.columns() {
      let column_width = Self::column_width(entries);
      if x >= column_x && x < column_x + column_width {
        let row = ((y - self.y) / Self::ROW_HEIGHT) as usize;
        let Some(entry) = entries.get(row) else {
          return TrayMenuHit::Outside;
        };
        path.push(row);
        if entry.separator || (!entry.enabled && entry.children.is_empty()) {
          return TrayMenuHit::Outside;
        }
        if !entry.children.is_empty() {
          return TrayMenuHit::OpenSubmenu { path };
        }
        return TrayMenuHit::Activate { id: entry.id };
      }
      let Some(&open_index) = self.open_path.get(path.len()) else {
        break;
      };
      path.push(open_index);
      column_x += column_width;
    }
    TrayMenuHit::Outside
  }
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
  pub battery: Segment,
  pub clock: Segment,
}
