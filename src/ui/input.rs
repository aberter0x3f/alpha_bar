use crate::{
  state::{Click, Segment, Side, TrayMenuPopup},
  ui::layout::{LEFT_EDGE_PAD, RIGHT_EDGE_PAD, segment_width},
};

pub fn click_target(
  width: usize,
  bar_height: usize,
  x: f64,
  y: f64,
  segments: &[Segment],
  _menu: Option<&TrayMenuPopup>,
) -> Option<Click> {
  click_target_with_origin(width, bar_height, x, y, segments, _menu).map(|(click, _)| click)
}

pub fn click_target_with_origin(
  width: usize,
  bar_height: usize,
  x: f64,
  y: f64,
  segments: &[Segment],
  _menu: Option<&TrayMenuPopup>,
) -> Option<(Click, f64)> {
  (y < bar_height as f64).then(|| click_at_with_origin(width, x, segments))?
}

pub fn click_at(width: usize, x: f64, segments: &[Segment]) -> Option<Click> {
  click_at_with_origin(width, x, segments).map(|(click, _)| click)
}

pub fn click_at_with_origin(width: usize, x: f64, segments: &[Segment]) -> Option<(Click, f64)> {
  let right_width = segments
    .iter()
    .filter(|segment| segment.side == Side::Right)
    .map(input_segment_width)
    .sum::<f64>();
  let mut left = LEFT_EDGE_PAD;
  let mut right = width as f64 - RIGHT_EDGE_PAD - right_width;

  for segment in segments {
    let width = input_segment_width(segment);
    let start = match segment.side {
      Side::Left => {
        let start = left;
        left += width;
        start
      }
      Side::Right => {
        let start = right;
        right += width;
        start
      }
    };
    if (start..start + width).contains(&x) {
      return segment.click.clone().map(|click| (click, start));
    }
  }
  None
}

fn input_segment_width(segment: &Segment) -> f64 {
  segment_width(segment, |text| text.chars().count() as f64 * 10.0)
}
