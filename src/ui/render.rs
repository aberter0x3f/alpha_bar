use crate::state::{Segment, Side};
use crate::{
  config::STYLE,
  style::{argb, underline},
};

pub fn render_segments(width: usize, height: usize, segments: &[Segment]) -> Vec<u8> {
  render_segments_scaled(width, height, 1, segments)
}

pub fn render_segments_scaled(
  width: usize,
  height: usize,
  scale: u32,
  segments: &[Segment],
) -> Vec<u8> {
  let scale = scale.max(1) as usize;
  let mut surface = cairo::ImageSurface::create(
    cairo::Format::ARgb32,
    (width * scale) as i32,
    (height * scale) as i32,
  )
  .expect("create cairo surface");
  let cr = cairo::Context::new(&surface).expect("create cairo context");
  cr.scale(scale as f64, scale as f64);
  set_color(&cr, argb(STYLE.background));
  cr.paint().expect("paint background");

  let mut left = 8.0;
  for segment in segments.iter().filter(|segment| segment.side == Side::Left) {
    let segment_width = segment_width(&cr, segment);
    draw_segment(&cr, segment, left, segment_width, height);
    left += segment_width;
  }

  let right_width = segments
    .iter()
    .filter(|segment| segment.side == Side::Right)
    .map(|segment| segment_width(&cr, segment))
    .sum::<f64>();
  let mut right = width as f64 - 8.0 - right_width;
  for segment in segments
    .iter()
    .filter(|segment| segment.side == Side::Right)
  {
    let segment_width = segment_width(&cr, segment);
    draw_segment(&cr, segment, right, segment_width, height);
    right += segment_width;
  }

  drop(cr);
  surface.flush();
  surface.data().expect("read cairo surface").to_vec()
}

fn segment_width(cr: &cairo::Context, segment: &Segment) -> f64 {
  if segment.icon.is_some() {
    25.0
  } else if segment.side == Side::Left {
    (segment.text.chars().count() as f64 * 10.0 + 16.0).max(40.0)
  } else {
    text_width(cr, &segment.text) + 16.0
  }
}

fn text_width(cr: &cairo::Context, text: &str) -> f64 {
  let layout = pangocairo::functions::create_layout(cr);
  layout.set_text(text);
  layout.set_font_description(Some(&font_description()));
  (layout.pixel_extents().1.width() as f64).max(4.0)
}

fn draw_segment(cr: &cairo::Context, segment: &Segment, x: f64, segment_width: f64, height: usize) {
  if let Some(icon) = &segment.icon {
    draw_icon(cr, x, 6.0, icon);
  } else {
    draw_text_centered(cr, x, 4.0, segment_width - 8.0, &segment.text);
  }
  set_color(cr, argb(underline(segment.class)));
  cr.rectangle(x, height as f64 - 2.0, segment_width - 8.0, 2.0);
  cr.fill().expect("fill underline");
}

fn draw_icon(cr: &cairo::Context, x: f64, y: f64, icon: &crate::state::Icon) {
  let Ok(surface) = cairo::ImageSurface::create_for_data(
    icon.pixels.clone(),
    cairo::Format::ARgb32,
    icon.width as i32,
    icon.height as i32,
    icon.width as i32 * 4,
  ) else {
    return;
  };
  cr.save().expect("save icon context");
  cr.translate(x, y);
  let scale = 20.0 / icon.height as f64;
  cr.scale(scale, scale);
  let pattern = cairo::SurfacePattern::create(&surface);
  pattern.set_filter(cairo::Filter::Best);
  cr.set_source(&pattern).expect("set icon source");
  cr.paint().expect("paint icon");
  cr.restore().expect("restore icon context");
}

fn draw_text_centered(cr: &cairo::Context, x: f64, y: f64, width: f64, text: &str) {
  let layout = pangocairo::functions::create_layout(cr);
  layout.set_text(text);
  layout.set_font_description(Some(&font_description()));
  let ink = layout.pixel_extents().0;
  set_color(cr, argb(STYLE.foreground));
  cr.move_to(x + (width - ink.width() as f64) / 2.0 - ink.x() as f64, y);
  pangocairo::functions::show_layout(cr, &layout);
}

fn font_description() -> pango::FontDescription {
  pango::FontDescription::from_string(&format!("{} 16", STYLE.font))
}

fn set_color(cr: &cairo::Context, argb: u32) {
  let r = ((argb >> 16) & 0xff) as f64 / 255.0;
  let g = ((argb >> 8) & 0xff) as f64 / 255.0;
  let b = (argb & 0xff) as f64 / 255.0;
  cr.set_source_rgb(r, g, b);
}
