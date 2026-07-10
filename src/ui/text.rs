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

pub(crate) fn font_description(font: &str) -> pango::FontDescription {
  let mut font = pango::FontDescription::from_string(font);
  font.set_absolute_size(16.0 * pango::SCALE as f64);
  font
}

pub(crate) struct TextMeasure {
  cr: cairo::Context,
  font: pango::FontDescription,
}

impl TextMeasure {
  pub(crate) fn new(font: &str) -> Self {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1, 1)
      .expect("create text measure surface");
    let cr = cairo::Context::new(&surface).expect("create text measure context");
    Self {
      cr,
      font: font_description(font),
    }
  }

  pub(crate) fn width(&self, text: &str) -> f64 {
    text_width(&self.cr, &self.font, text)
  }
}

pub(crate) fn text_width(cr: &cairo::Context, font: &pango::FontDescription, text: &str) -> f64 {
  let layout = pangocairo::functions::create_layout(cr);
  layout.set_text(text);
  layout.set_font_description(Some(font));
  (layout.pixel_extents().1.width() as f64).max(4.0)
}
