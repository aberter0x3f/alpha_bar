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
  let surface =
    cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).expect("create input measure surface");
  let cr = cairo::Context::new(&surface).expect("create input measure context");
  let font = text::font_description(STYLE.font);
  for rect in layout_segments(width, segments, |value| text::text_width(&cr, &font, value)) {
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
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1).unwrap();
    let cr = cairo::Context::new(&surface).unwrap();
    let font = text::font_description("Sans");

    assert!(text::text_width(&cr, &font, "WWWW") > text::text_width(&cr, &font, "iiii"));
  }
}
