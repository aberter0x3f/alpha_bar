/*
  This file is part of Alpha Bar.

  Alpha Bar is free software: you can redistribute it and/or modify it under the terms of the GNU
  General Public License as published by the Free Software Foundation, either version 3 of the
  License, or (at your option) any later version.

  Alpha Bar is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
  even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
  General Public License for more details.

  You should have received a copy of the GNU General Public License along with Alpha Bar. If not,
  see <https://www.gnu.org/licenses/>.
*/

use crate::{
  config::STYLE,
  state::{Click, Segment},
  ui::{layout::layout_segments, text},
};

pub fn click_target_with_origin(
  width: usize,
  bar_height: usize,
  x: f64,
  y: f64,
  segments: &[&Segment],
) -> Option<(Click, f64)> {
  (y < bar_height as f64).then(|| click_at_with_origin(width, x, segments))?
}

pub fn click_at_with_origin(width: usize, x: f64, segments: &[&Segment]) -> Option<(Click, f64)> {
  let measure = text::TextMeasure::new(STYLE.font);
  for rect in layout_segments(width, segments, |value| measure.width(value)) {
    if (rect.x..rect.x + rect.width).contains(&x) {
      return rect.segment.click.clone().map(|click| (click, rect.x));
    }
  }
  None
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
    let refs = segments.iter().collect::<Vec<_>>();

    assert_eq!(click_at_with_origin(200, 10.0, &refs), None);
    assert_eq!(
      click_at_with_origin(200, 50.0, &refs),
      Some((Click::Tray("addr".into()), 34.0))
    );
  }

  #[test]
  fn text_width_uses_font_metrics() {
    let measure = text::TextMeasure::new("Sans");

    assert!(measure.width("WWWW") > measure.width("iiii"));
  }
}
