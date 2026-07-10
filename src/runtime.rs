use std::sync::mpsc::{Receiver, Sender};

use crate::{
  modules::{audio, clock, cpu, memory, net_speed, network, niri, power, temperature, tray},
  state::{Class, Segment, Side, Snapshot, TrayMenuPopup},
};

pub enum Update {
  Workspaces(Vec<Segment>),
  Tray(Vec<Segment>),
  TrayMenu(Option<TrayMenuPopup>),
  Network(Segment),
  NetSpeed(Segment),
  Audio(Segment),
  Cpu(Segment),
  Memory(Segment),
  Temperature(Segment),
  Battery(Option<Segment>),
  Clock(Segment),
}

pub enum RuntimeCommand {
  TrayActivate {
    address: String,
    context_menu: bool,
    x: i32,
    y: i32,
    menu_x: i32,
  },
  TrayMenuItem {
    address: String,
    path: String,
    id: i32,
  },
  TrayMenuClose,
  TrayScale(u32),
}

pub struct RuntimeState {
  snapshot: Snapshot,
}

impl Default for RuntimeState {
  fn default() -> Self {
    Self {
      snapshot: Snapshot {
        workspaces: vec![Segment::new(Side::Left, Class::Active, "1")],
        tray: Vec::new(),
        tray_menu: None,
        network: network::segment("󰲛 Disconnected".into()),
        net_speed: net_speed::default_segment(),
        audio: audio::segment("󰖁 0%".into()),
        cpu: cpu::segment(0),
        memory: memory::segment(0),
        temperature: temperature::segment(0),
        battery: None,
        clock: clock::segment(clock::clock_text(0, 0)),
      },
    }
  }
}

impl RuntimeState {
  pub fn apply(&mut self, update: Update) {
    match update {
      Update::Workspaces(v) => self.snapshot.workspaces = v,
      Update::Tray(v) => self.snapshot.tray = v,
      Update::TrayMenu(v) => self.snapshot.tray_menu = v,
      Update::Network(v) => self.snapshot.network = v,
      Update::NetSpeed(v) => self.snapshot.net_speed = v,
      Update::Audio(v) => self.snapshot.audio = v,
      Update::Cpu(v) => self.snapshot.cpu = v,
      Update::Memory(v) => self.snapshot.memory = v,
      Update::Temperature(v) => self.snapshot.temperature = v,
      Update::Battery(v) => self.snapshot.battery = v,
      Update::Clock(v) => self.snapshot.clock = v,
    }
  }

  pub fn snapshot(&self) -> &Snapshot {
    &self.snapshot
  }
}

pub fn spawn_workers(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  network::spawn(tx.clone());
  audio::spawn(tx.clone());
  memory::spawn(tx.clone());
  temperature::spawn(tx.clone());
  power::spawn(tx.clone());
  clock::spawn(tx.clone());
  net_speed::spawn(tx.clone());
  niri::spawn(tx.clone());
  tray::spawn(tx.clone(), cmd_rx);
  cpu::spawn(tx);
}
