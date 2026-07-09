use alpha_bar::clock::clock_text;

#[test]
fn clock() {
  assert_eq!(clock_text(9, 5), "󰥔 09:05");
}
