use crate::state::{Segment, Side};

pub const LEFT_EDGE_PAD: f64 = 2.0;
pub const RIGHT_EDGE_PAD: f64 = 8.0;
pub const RIGHT_GAP: f64 = 16.0;
const ICON_SEGMENT_WIDTH: f64 = 25.0;
const WORKSPACE_MIN_WIDTH: f64 = 32.0;
const LEFT_TEXT_PAD: f64 = 8.0;

pub fn segment_width(segment: &Segment, text_width: impl Fn(&str) -> f64) -> f64 {
  if segment.icon.is_some() {
    ICON_SEGMENT_WIDTH
  } else if segment.side == Side::Left {
    (text_width(&segment.text) + LEFT_TEXT_PAD).max(WORKSPACE_MIN_WIDTH)
  } else {
    text_width(&segment.text) + RIGHT_GAP
  }
}
