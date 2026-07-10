use alpha_bar::{
  runtime::{RuntimeState, Update, network_text},
  state::{Class, Segment, Side},
};

#[test]
fn network_states() {
  assert_eq!(network_text(None, "up"), "󰲛 Disconnected");
  assert_eq!(network_text(Some("enp4s0"), "up"), "󰈀 enp4s0");
  assert_eq!(network_text(Some("wlan0"), "up"), "󰤨 wlan0");
  assert_eq!(network_text(Some("wlan0"), "down"), "wlan0 (No IP)");
}

#[test]
fn wifi_text_uses_ssid_when_available() {
  assert_eq!(
    alpha_bar::runtime::wifi_network_text("wlan0", Some("CafeNet")),
    "󰤨 CafeNet"
  );
  assert_eq!(
    alpha_bar::runtime::wifi_network_text("wlan0", None),
    "󰤨 wlan0"
  );
}

#[test]
fn independent_updates() {
  let mut state = RuntimeState::default();
  state.apply(Update::Audio("󰖁 0%".into()));
  state.apply(Update::Workspaces(vec![
    Segment::new(Side::Left, Class::Normal, "1"),
    Segment::new(Side::Left, Class::Active, "2"),
  ]));
  state.apply(Update::Tray(vec![Segment::new(
    Side::Left,
    Class::Normal,
    "tray",
  )]));
  state.apply(Update::Cpu(88));
  state.apply(Update::Battery("󰂎 9%".into(), Class::Critical));

  let snapshot = state.snapshot();
  assert_eq!(snapshot.audio, "󰖁 0%");
  assert_eq!(snapshot.workspaces[1].class, Class::Active);
  assert_eq!(snapshot.tray[0].text, "tray");
  assert_eq!(snapshot.cpu, 88);
  assert_eq!(snapshot.battery, "󰂎 9%");
  assert_eq!(snapshot.memory, 0);
}
