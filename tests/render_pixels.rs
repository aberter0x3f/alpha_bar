use alpha_bar::{
  state::{Class, Icon, Segment, Side},
  ui::render::{render_segments, render_segments_scaled},
};

#[test]
fn bar_pixels() {
  let pixels = render_segments(64, 32, &[Segment::new(Side::Left, Class::Active, "1")]);
  assert_eq!(pixels.len(), 64 * 32 * 4);
  assert!(
    pixels
      .chunks_exact(4)
      .any(|px| px != [0x52, 0x42, 0x3b, 0xff])
  );
}

#[test]
fn segment_underlines() {
  let pixels = render_segments(
    240,
    32,
    &[
      Segment::new(Side::Left, Class::Active, "1"),
      Segment::new(Side::Right, Class::Critical, "󰲛 Disconnected"),
    ],
  );
  let active = ((30 * 240 + 8) * 4)..((30 * 240 + 9) * 4);
  let critical = ((30 * 240 + 206) * 4)..((30 * 240 + 207) * 4);
  assert_eq!(&pixels[active], [0xbb, 0xbc, 0x8f, 0xff]);
  assert_eq!(&pixels[critical], [0x6a, 0x61, 0xbf, 0xff]);
}

#[test]
fn right_segments_keep_config_order() {
  let pixels = render_segments(
    120,
    32,
    &[
      Segment::new(Side::Right, Class::Warning, "A"),
      Segment::new(Side::Right, Class::Critical, "B"),
    ],
  );

  let warning = [0x70, 0x87, 0xd0, 0xff];
  let critical = [0x6a, 0x61, 0xbf, 0xff];
  let row = pixels[30 * 120 * 4..31 * 120 * 4].chunks_exact(4);
  let first_warning = row
    .clone()
    .position(|px| px == warning)
    .expect("warning underline");
  let first_critical = row
    .clone()
    .position(|px| px == critical)
    .expect("critical underline");

  assert!(first_warning < first_critical);
}

#[test]
fn tray_icon_pixels() {
  let pixels = render_segments(
    40,
    32,
    &[Segment::icon(
      Side::Left,
      Class::Normal,
      Icon {
        width: 1,
        height: 1,
        pixels: vec![0x00, 0x00, 0xff, 0xff],
      },
    )],
  );

  let icon_pixel = ((6 * 40 + 8) * 4)..((6 * 40 + 9) * 4);
  assert_eq!(&pixels[icon_pixel], [0x00, 0x00, 0xff, 0xff]);
}

#[test]
fn workspace_min_width() {
  let pixels = render_segments(80, 32, &[Segment::new(Side::Left, Class::Active, "1")]);
  let right_edge = ((30 * 80 + 39) * 4)..((30 * 80 + 40) * 4);
  assert_eq!(&pixels[right_edge], [0xbb, 0xbc, 0x8f, 0xff]);
}

#[test]
fn default_bar_has_foreground_pixels() {
  let pixels = render_segments(320, 32, &[Segment::new(Side::Left, Class::Active, "1")]);
  assert!(
    pixels
      .chunks_exact(4)
      .any(|px| px == [0xf4, 0xef, 0xec, 0xff])
  );
}

#[test]
fn scaled_render_uses_physical_pixels() {
  let pixels = render_segments_scaled(80, 32, 2, &[Segment::new(Side::Left, Class::Active, "1")]);
  assert_eq!(pixels.len(), 160 * 64 * 4);
  let underline = ((60 * 160 + 16) * 4)..((60 * 160 + 17) * 4);
  assert_eq!(&pixels[underline], [0xbb, 0xbc, 0x8f, 0xff]);
}

#[test]
fn workspace_text_is_centered_over_underline() {
  let pixels = render_segments(80, 32, &[Segment::new(Side::Left, Class::Active, "1")]);
  let background = [0x52, 0x42, 0x3b, 0xff];
  let underline = [0xbb, 0xbc, 0x8f, 0xff];
  let mut min_x = usize::MAX;
  let mut max_x = 0;
  for (i, px) in pixels.chunks_exact(4).enumerate() {
    let y = i / 80;
    if y < 28 && px != background && px != underline {
      let x = i % 80;
      min_x = min_x.min(x);
      max_x = max_x.max(x);
    }
  }
  let text_center = (min_x + max_x) / 2;
  assert!(
    (20..=27).contains(&text_center),
    "text center {text_center}, ink {min_x}..{max_x}"
  );
}
