use crate::{
  state::{Click, Segment},
  ui::layout::layout_segments,
};

pub fn click_target_with_origin(
  width: usize,
  bar_height: usize,
  x: f64,
  y: f64,
  segments: &[Segment],
) -> Option<(Click, f64)> {
  (y < bar_height as f64).then(|| click_at_with_origin(width, x, segments))?
}

pub fn click_at_with_origin(width: usize, x: f64, segments: &[Segment]) -> Option<(Click, f64)> {
  for rect in layout_segments(width, segments, input_text_width) {
    if (rect.x..rect.x + rect.width).contains(&x) {
      return rect.segment.click.clone().map(|click| (click, rect.x));
    }
  }
  None
}

fn input_text_width(text: &str) -> f64 {
  text.chars().count() as f64 * 10.0
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::state::{Class, Segment, Side};

  #[test]
  fn tray_click_hit() {
    let segments = vec![
      Segment::new(Side::Left, Class::Active, "1"),
      Segment::new(Side::Left, Class::Normal, "tray").click(Click::Tray("addr".into())),
    ];

    assert_eq!(click_at_with_origin(200, 10.0, &segments), None);
    assert_eq!(
      click_at_with_origin(200, 50.0, &segments),
      Some((Click::Tray("addr".into()), 34.0))
    );
  }
}
