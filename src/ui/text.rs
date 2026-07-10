pub(crate) fn font_description(font: &str) -> pango::FontDescription {
  let mut font = pango::FontDescription::from_string(font);
  font.set_absolute_size(16.0 * pango::SCALE as f64);
  font
}

pub(crate) fn text_width(cr: &cairo::Context, font: &pango::FontDescription, text: &str) -> f64 {
  let layout = pangocairo::functions::create_layout(cr);
  layout.set_text(text);
  layout.set_font_description(Some(font));
  (layout.pixel_extents().1.width() as f64).max(4.0)
}
