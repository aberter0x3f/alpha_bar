use crate::state::{Segment, Side, TrayMenuEntry, TrayMenuHit, TrayMenuPopup};

pub const LEFT_EDGE_PAD: f64 = 2.0;
pub const RIGHT_EDGE_PAD: f64 = 8.0;
pub const RIGHT_GAP: f64 = 16.0;
const ICON_SEGMENT_WIDTH: f64 = 25.0;
const WORKSPACE_MIN_WIDTH: f64 = 32.0;
const LEFT_TEXT_PAD: f64 = 8.0;
pub(crate) const MENU_ROW_HEIGHT: f64 = 24.0;
const MENU_ICON_WIDTH: f64 = 28.0;
const MENU_ARROW_WIDTH: f64 = 18.0;
const MENU_MIN_WIDTH: f64 = 160.0;

pub(crate) struct SegmentRect<'a> {
  pub segment: &'a Segment,
  pub x: f64,
  pub width: f64,
}

pub(crate) fn layout_segments<'a>(
  width: usize,
  segments: &'a [&'a Segment],
  text_width: impl Fn(&str) -> f64,
) -> Vec<SegmentRect<'a>> {
  let widths = segments
    .iter()
    .map(|segment| segment_width(segment, &text_width))
    .collect::<Vec<_>>();
  let right_width = segments
    .iter()
    .zip(&widths)
    .filter(|(segment, _)| segment.side == Side::Right)
    .map(|(_, width)| width)
    .sum::<f64>();
  let mut left = LEFT_EDGE_PAD;
  let mut right = width as f64 - RIGHT_EDGE_PAD - right_width;

  segments
    .iter()
    .zip(widths)
    .map(|(segment, segment_width)| {
      let x = match segment.side {
        Side::Left => {
          let x = left;
          left += segment_width;
          x
        }
        Side::Right => {
          let x = right;
          right += segment_width;
          x
        }
      };
      SegmentRect {
        segment,
        x,
        width: segment_width,
      }
    })
    .collect()
}

pub(crate) fn segment_width(segment: &Segment, text_width: impl Fn(&str) -> f64) -> f64 {
  if segment.icon.is_some() {
    ICON_SEGMENT_WIDTH
  } else if segment.side == Side::Left {
    (text_width(&segment.text) + LEFT_TEXT_PAD).max(WORKSPACE_MIN_WIDTH)
  } else {
    text_width(&segment.text) + RIGHT_GAP
  }
}

pub(crate) fn menu_columns(menu: &TrayMenuPopup) -> Vec<&[TrayMenuEntry]> {
  let mut columns = vec![menu.entries.as_slice()];
  let mut entries = menu.entries.as_slice();
  for &index in &menu.open_path {
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

pub(crate) fn menu_column_width(entries: &[TrayMenuEntry]) -> f64 {
  entries
    .iter()
    .map(|entry| {
      if entry.separator {
        MENU_MIN_WIDTH
      } else {
        entry.label.chars().count() as f64 * 10.0 + MENU_ICON_WIDTH + MENU_ARROW_WIDTH + 16.0
      }
    })
    .fold(MENU_MIN_WIDTH, f64::max)
}

pub(crate) fn menu_width(menu: &TrayMenuPopup) -> f64 {
  menu_columns(menu)
    .iter()
    .map(|entries| menu_column_width(entries))
    .sum()
}

pub(crate) fn menu_height(menu: &TrayMenuPopup) -> f64 {
  menu_columns(menu)
    .iter()
    .map(|entries| entries.len() as f64 * MENU_ROW_HEIGHT)
    .fold(0.0, f64::max)
}

pub(crate) fn menu_hit(menu: &TrayMenuPopup, x: f64, y: f64) -> TrayMenuHit {
  if x < menu.x || y < menu.y || x >= menu.x + menu_width(menu) || y >= menu.y + menu_height(menu) {
    return TrayMenuHit::Outside;
  }
  let mut column_x = menu.x;
  let mut path = Vec::new();
  for entries in menu_columns(menu) {
    let column_width = menu_column_width(entries);
    if x >= column_x && x < column_x + column_width {
      let row = ((y - menu.y) / MENU_ROW_HEIGHT) as usize;
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
    let Some(&open_index) = menu.open_path.get(path.len()) else {
      break;
    };
    path.push(open_index);
    column_x += column_width;
  }
  TrayMenuHit::Outside
}

pub(crate) fn menu_hover(menu: &TrayMenuPopup, x: f64, y: f64) -> Option<Vec<usize>> {
  if x < menu.x || y < menu.y || x >= menu.x + menu_width(menu) || y >= menu.y + menu_height(menu) {
    return None;
  }
  let mut column_x = menu.x;
  let mut path = Vec::new();
  for entries in menu_columns(menu) {
    let column_width = menu_column_width(entries);
    if x >= column_x && x < column_x + column_width {
      let row = ((y - menu.y) / MENU_ROW_HEIGHT) as usize;
      let entry = entries.get(row)?;
      if entry.separator {
        return None;
      }
      path.push(row);
      return Some(path);
    }
    let Some(&open_index) = menu.open_path.get(path.len()) else {
      break;
    };
    path.push(open_index);
    column_x += column_width;
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn menu_click_hit() {
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

    assert_eq!(menu_hit(&menu, 24.0, 34.0), TrayMenuHit::Activate { id: 7 });
  }

  #[test]
  fn submenu_hit() {
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
      menu_hit(&menu, 4.0, 36.0),
      TrayMenuHit::OpenSubmenu { path: vec![0] }
    );
    menu.open_path = vec![0];
    assert_eq!(
      menu_hit(&menu, menu_column_width(&menu.entries) + 4.0, 36.0),
      TrayMenuHit::Activate { id: 2 }
    );
  }

  #[test]
  fn menu_hover_path() {
    let menu = TrayMenuPopup {
      address: ":1.2/StatusNotifierItem".into(),
      path: "/menu".into(),
      x: 0.0,
      y: 32.0,
      entries: vec![
        TrayMenuEntry {
          id: 1,
          label: "Open".into(),
          enabled: true,
          separator: false,
          icon: None,
          children: Vec::new(),
        },
        TrayMenuEntry {
          id: 2,
          label: "Disabled".into(),
          enabled: false,
          separator: false,
          icon: None,
          children: Vec::new(),
        },
      ],
      open_path: Vec::new(),
    };

    assert_eq!(menu_hover(&menu, 4.0, 36.0), Some(vec![0]));
    assert_eq!(menu_hover(&menu, 4.0, 60.0), Some(vec![1]));
    assert_eq!(menu_hover(&menu, 4.0, 90.0), None);
  }
}
