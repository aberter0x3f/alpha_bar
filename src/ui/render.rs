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
  config::{BAR, STYLE},
  state::{Segment, Side, TrayMenuPopup},
  style::{argb, underline},
  ui::layout::{MENU_ROW_HEIGHT, RIGHT_GAP, layout_segments, menu_column_width, menu_columns},
  ui::text::{font_description, text_width},
};

pub(crate) fn render_ui_scaled(
  width: usize,
  height: usize,
  scale: u32,
  segments: &[&Segment],
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
  let font = font_description(STYLE.font);

  for rect in layout_segments(width, segments, |text| text_width(&cr, &font, text)) {
    draw_segment(&cr, &font, rect.segment, rect.x, rect.width);
  }

  if let Some(menu) = menu {
    draw_menu(&cr, &font, menu, None);
  }

  drop(cr);
  surface.flush();
  surface.data().expect("read cairo surface").to_vec()
}

pub(crate) fn render_menu_scaled(
  width: usize,
  height: usize,
  scale: u32,
  menu: &TrayMenuPopup,
  hover_path: Option<&[usize]>,
  origin_x: f64,
  origin_y: f64,
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
  let font = font_description(STYLE.font);
  cr.translate(-origin_x, -origin_y);
  draw_menu(&cr, &font, menu, hover_path);
  drop(cr);
  surface.flush();
  surface.data().expect("read cairo surface").to_vec()
}

fn draw_menu(
  cr: &cairo::Context,
  font: &pango::FontDescription,
  menu: &TrayMenuPopup,
  hover_path: Option<&[usize]>,
) {
  let mut x = menu.x;
  let mut path_prefix = Vec::new();
  for entries in menu_columns(menu) {
    let width = menu_column_width(entries, &|text| text_width(cr, font, text));
    set_color(cr, argb(STYLE.background));
    cr.rectangle(x, menu.y, width, entries.len() as f64 * MENU_ROW_HEIGHT);
    cr.fill().expect("fill tray menu");
    for (index, entry) in entries.iter().enumerate() {
      let y = menu.y + index as f64 * MENU_ROW_HEIGHT;
      let mut path = path_prefix.clone();
      path.push(index);
      if entry.separator {
        set_color(cr, argb(STYLE.border_normal));
        cr.rectangle(x + 8.0, y + 11.0, width - 16.0, 1.0);
        cr.fill().expect("draw tray menu separator");
        continue;
      }
      if hover_path.is_some_and(|hover| hover == path.as_slice()) {
        set_color(cr, argb(STYLE.border_normal));
        cr.rectangle(x, y, width, MENU_ROW_HEIGHT);
        cr.fill().expect("draw tray menu hover");
      }
      if let Some(icon) = &entry.icon {
        draw_icon(cr, x + 4.0, centered_y(y, 20.0), icon);
      }
      set_color(
        cr,
        argb(if entry.enabled {
          STYLE.foreground
        } else {
          STYLE.border_normal
        }),
      );
      let layout = pangocairo::functions::create_layout(cr);
      layout.set_text(&entry.label);
      layout.set_font_description(Some(font));
      cr.move_to(x + 32.0, text_y(y, &layout));
      pangocairo::functions::show_layout(cr, &layout);
      if !entry.children.is_empty() {
        let layout = pangocairo::functions::create_layout(cr);
        layout.set_text(">");
        layout.set_font_description(Some(font));
        cr.move_to(x + width - 14.0, text_y(y, &layout));
        pangocairo::functions::show_layout(cr, &layout);
      }
    }
    if let Some(&open_index) = menu.open_path.get(path_prefix.len()) {
      path_prefix.push(open_index);
    }
    x += width;
  }
}

fn centered_y(row_y: f64, height: f64) -> f64 {
  row_y + (MENU_ROW_HEIGHT - height) / 2.0
}

fn text_y(row_y: f64, layout: &pango::Layout) -> f64 {
  let extents = layout.pixel_extents().1;
  centered_y(row_y, extents.height() as f64) - extents.y() as f64
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
  let mut pixels = icon_surface_pixels(icon);
  // Cairo requires mutable image data; paint from a local copy so borrowed snapshot icons remain immutable.
  let Ok(surface) = (unsafe {
    cairo::ImageSurface::create_for_data_unsafe(
      pixels.as_mut_ptr(),
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

fn icon_surface_pixels(icon: &crate::state::Icon) -> Vec<u8> {
  icon.pixels.clone()
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

fn set_color(cr: &cairo::Context, argb: u32) {
  let r = ((argb >> 16) & 0xff) as f64 / 255.0;
  let g = ((argb >> 8) & 0xff) as f64 / 255.0;
  let b = (argb & 0xff) as f64 / 255.0;
  cr.set_source_rgb(r, g, b);
}

#[cfg(test)]
mod tests {
  use crate::state::Icon;

  use super::*;

  #[test]
  fn icon_pixels_copied() {
    let icon = Icon {
      width: 1,
      height: 1,
      pixels: vec![1, 2, 3, 4],
    };

    let pixels = icon_surface_pixels(&icon);
    assert_eq!(pixels, icon.pixels);
    assert_ne!(pixels.as_ptr(), icon.pixels.as_ptr());
  }

  #[test]
  fn menu_row_centers_content() {
    assert_eq!(centered_y(32.0, 20.0), 34.0);
    assert_eq!(centered_y(32.0, 12.0), 38.0);
  }
}
