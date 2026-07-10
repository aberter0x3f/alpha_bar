use crate::{
  config::{BAR, STYLE},
  state::{Segment, Side, TrayMenuPopup},
  style::{argb, underline},
  ui::layout::{LEFT_EDGE_PAD, RIGHT_EDGE_PAD, RIGHT_GAP, segment_width as layout_segment_width},
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
  render_ui_scaled(width, height, scale, segments, None)
}

pub fn render_ui_scaled(
  width: usize,
  height: usize,
  scale: u32,
  segments: &[Segment],
  menu: Option<&TrayMenuPopup>,
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
  cr.rectangle(
    0.0,
    0.0,
    width as f64,
    height.min(BAR.height as usize) as f64,
  );
  cr.fill().expect("paint background");
  let font = font_description();

  let mut left = LEFT_EDGE_PAD;
  for segment in segments.iter().filter(|segment| segment.side == Side::Left) {
    let segment_width = segment_width(&cr, &font, segment);
    draw_segment(&cr, &font, segment, left, segment_width);
    left += segment_width;
  }

  let right_width = segments
    .iter()
    .filter(|segment| segment.side == Side::Right)
    .map(|segment| segment_width(&cr, &font, segment))
    .sum::<f64>();
  let mut right = width as f64 - RIGHT_EDGE_PAD - right_width;
  for segment in segments
    .iter()
    .filter(|segment| segment.side == Side::Right)
  {
    let segment_width = segment_width(&cr, &font, segment);
    draw_segment(&cr, &font, segment, right, segment_width);
    right += segment_width;
  }

  if let Some(menu) = menu {
    draw_menu(&cr, &font, menu);
  }

  drop(cr);
  surface.flush();
  surface.data().expect("read cairo surface").to_vec()
}

pub fn render_menu_scaled(
  width: usize,
  height: usize,
  scale: u32,
  menu: &TrayMenuPopup,
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
  cr.set_operator(cairo::Operator::Clear);
  cr.rectangle(0.0, 0.0, width as f64, height as f64);
  cr.fill().expect("clear menu surface");
  cr.set_operator(cairo::Operator::Over);
  let font = font_description();
  draw_menu(&cr, &font, menu);
  drop(cr);
  surface.flush();
  surface.data().expect("read cairo surface").to_vec()
}

fn draw_menu(cr: &cairo::Context, font: &pango::FontDescription, menu: &TrayMenuPopup) {
  let mut x = menu.x;
  for entries in menu.columns() {
    let width = TrayMenuPopup::column_width(entries);
    set_color(cr, argb(STYLE.background));
    cr.rectangle(
      x,
      menu.y,
      width,
      entries.len() as f64 * TrayMenuPopup::ROW_HEIGHT,
    );
    cr.fill().expect("fill tray menu");
    for (index, entry) in entries.iter().enumerate() {
      let y = menu.y + index as f64 * TrayMenuPopup::ROW_HEIGHT;
      if entry.separator {
        set_color(cr, argb(STYLE.border_normal));
        cr.rectangle(x + 8.0, y + 11.0, width - 16.0, 1.0);
        cr.fill().expect("draw tray menu separator");
        continue;
      }
      if let Some(icon) = &entry.icon {
        draw_icon(cr, x + 4.0, y + 2.0, icon);
      }
      set_color(
        cr,
        argb(if entry.enabled {
          STYLE.foreground
        } else {
          STYLE.border_normal
        }),
      );
      cr.move_to(x + 32.0, y + 4.0);
      let layout = pangocairo::functions::create_layout(cr);
      layout.set_text(&entry.label);
      layout.set_font_description(Some(font));
      pangocairo::functions::show_layout(cr, &layout);
      if !entry.children.is_empty() {
        cr.move_to(x + width - 14.0, y + 4.0);
        let layout = pangocairo::functions::create_layout(cr);
        layout.set_text(">");
        layout.set_font_description(Some(font));
        pangocairo::functions::show_layout(cr, &layout);
      }
    }
    x += width;
  }
}

fn segment_width(cr: &cairo::Context, font: &pango::FontDescription, segment: &Segment) -> f64 {
  layout_segment_width(segment, |text| text_width(cr, font, text))
}

fn text_width(cr: &cairo::Context, font: &pango::FontDescription, text: &str) -> f64 {
  let layout = pangocairo::functions::create_layout(cr);
  layout.set_text(text);
  layout.set_font_description(Some(font));
  (layout.pixel_extents().1.width() as f64).max(4.0)
}

fn draw_segment(
  cr: &cairo::Context,
  font: &pango::FontDescription,
  segment: &Segment,
  x: f64,
  segment_width: f64,
) {
  if let Some(icon) = &segment.icon {
    draw_icon(cr, x + 1.0, 6.0, icon);
  } else {
    let (content_x, content_width) = if segment.side == Side::Right {
      (x + RIGHT_GAP, segment_width - RIGHT_GAP)
    } else {
      (x, segment_width)
    };
    draw_text_centered(cr, font, content_x, 4.0, content_width, &segment.text);
    set_color(cr, argb(underline(segment.class)));
    cr.rectangle(content_x, BAR.height as f64 - 2.0, content_width, 2.0);
    cr.fill().expect("fill underline");
  }
}

fn draw_icon(cr: &cairo::Context, x: f64, y: f64, icon: &crate::state::Icon) {
  if icon.height == 0 {
    return;
  }
  // The surface is used only for the immediate paint below and never escapes this function.
  let Ok(surface) = (unsafe {
    cairo::ImageSurface::create_for_data_unsafe(
      icon.pixels.as_ptr().cast_mut(),
      cairo::Format::ARgb32,
      icon.width as i32,
      icon.height as i32,
      icon.width as i32 * 4,
    )
  }) else {
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

fn draw_text_centered(
  cr: &cairo::Context,
  font: &pango::FontDescription,
  x: f64,
  y: f64,
  width: f64,
  text: &str,
) {
  let layout = pangocairo::functions::create_layout(cr);
  layout.set_text(text);
  layout.set_font_description(Some(font));
  let ink = layout.pixel_extents().0;
  set_color(cr, argb(STYLE.foreground));
  cr.move_to(x + (width - ink.width() as f64) / 2.0 - ink.x() as f64, y);
  pangocairo::functions::show_layout(cr, &layout);
}

fn font_description() -> pango::FontDescription {
  let mut font = pango::FontDescription::from_string(STYLE.font);
  font.set_absolute_size(16.0 * pango::SCALE as f64);
  font
}

fn set_color(cr: &cairo::Context, argb: u32) {
  let r = ((argb >> 16) & 0xff) as f64 / 255.0;
  let g = ((argb >> 8) & 0xff) as f64 / 255.0;
  let b = (argb & 0xff) as f64 / 255.0;
  cr.set_source_rgb(r, g, b);
}

#[cfg(test)]
mod tests {
  #[test]
  fn font_pixels() {
    let font = super::font_description();
    assert!(font.is_size_absolute());
    assert_eq!(font.size(), 16 * pango::SCALE);
  }
}
