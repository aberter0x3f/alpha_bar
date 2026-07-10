use crate::state::{Click, Segment, Side, TrayMenuPopup};

pub fn click_target(
  width: usize,
  bar_height: usize,
  x: f64,
  y: f64,
  segments: &[Segment],
  _menu: Option<&TrayMenuPopup>,
) -> Option<Click> {
  (y < bar_height as f64).then(|| click_at(width, x, segments))?
}

pub fn click_at(width: usize, x: f64, segments: &[Segment]) -> Option<Click> {
  let right_width = segments
    .iter()
    .filter(|segment| segment.side == Side::Right)
    .map(segment_width)
    .sum::<f64>();
  let mut left = 8.0;
  let mut right = width as f64 - 8.0 - right_width;

  for segment in segments {
    let width = segment_width(segment);
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
      return segment.click.clone();
    }
  }
  None
}

fn segment_width(segment: &Segment) -> f64 {
  if segment.icon.is_some() {
    25.0
  } else if segment.side == Side::Left {
    (segment.text.chars().count() as f64 * 10.0 + 16.0).max(40.0)
  } else {
    (segment.text.chars().count() as f64 * 10.0 + 16.0).max(20.0)
  }
}
