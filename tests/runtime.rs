use alpha_bar::{
  modules::{audio, cpu, power},
  runtime::{RuntimeState, Update},
  state::{Class, Segment, Side},
};

#[test]
fn independent_updates() {
  let mut state = RuntimeState::default();
  state.apply(Update::Audio(audio::segment("󰖁 0%".into())));
  state.apply(Update::Workspaces(vec![
    Segment::new(Side::Left, Class::Normal, "1"),
    Segment::new(Side::Left, Class::Active, "2"),
  ]));
  state.apply(Update::Tray(vec![Segment::new(
    Side::Left,
    Class::Normal,
    "tray",
  )]));
  state.apply(Update::Cpu(cpu::segment(88)));
  state.apply(Update::Battery(power::segment("󰂎 9%".into(), "critical")));

  let snapshot = state.snapshot();
  assert_eq!(snapshot.audio.text, "󰖁 0%");
  assert_eq!(snapshot.workspaces[1].class, Class::Active);
  assert_eq!(snapshot.tray[0].text, "tray");
  assert_eq!(snapshot.cpu.text, "█ 88%");
  assert_eq!(snapshot.battery.text, "󰂎 9%");
  assert_eq!(snapshot.memory.text, "󰍛 0%");
}
