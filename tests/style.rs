use alpha_bar::style::argb;

#[test]
fn hex_argb() {
  assert_eq!(argb("#3b4252"), 0xff3b4252);
  assert_eq!(argb("bad"), 0xff000000);
}
