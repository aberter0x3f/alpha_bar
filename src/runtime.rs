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

use std::sync::mpsc::{Receiver, Sender};

use crate::{
  modules::{audio, clock, cpu, memory, network, niri, power, temperature, tray},
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

impl Default for Snapshot {
  fn default() -> Self {
    Snapshot {
      workspaces: vec![Segment::new(Side::Left, Class::Active, "1")],
      tray: Vec::new(),
      tray_menu: None,
      network: network::segment("󰲛 Disconnected".into()),
      net_speed: network::default_speed_segment(),
      audio: audio::segment("󰖁 0%".into()),
      cpu: cpu::segment(0),
      memory: memory::segment(0),
      temperature: temperature::segment(0),
      battery: None,
      clock: clock::segment(clock::clock_text(0, 0)),
    }
  }
}

impl Snapshot {
  pub fn apply(&mut self, update: Update) {
    match update {
      Update::Workspaces(v) => self.workspaces = v,
      Update::Tray(v) => self.tray = v,
      Update::TrayMenu(v) => self.tray_menu = v,
      Update::Network(v) => self.network = v,
      Update::NetSpeed(v) => self.net_speed = v,
      Update::Audio(v) => self.audio = v,
      Update::Cpu(v) => self.cpu = v,
      Update::Memory(v) => self.memory = v,
      Update::Temperature(v) => self.temperature = v,
      Update::Battery(v) => self.battery = v,
      Update::Clock(v) => self.clock = v,
    }
  }
}

pub fn spawn_workers(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  network::spawn(tx.clone());
  audio::spawn(tx.clone());
  memory::spawn(tx.clone());
  temperature::spawn(tx.clone());
  power::spawn(tx.clone());
  clock::spawn(tx.clone());
  niri::spawn(tx.clone());
  tray::spawn(tx.clone(), cmd_rx);
  cpu::spawn(tx);
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    modules::{audio, cpu, power},
    state::{Class, Side},
  };

  #[test]
  fn independent_updates() {
    let mut snapshot = Snapshot::default();
    snapshot.apply(Update::Audio(audio::segment("󰖁 0%".into())));
    snapshot.apply(Update::Workspaces(vec![
      Segment::new(Side::Left, Class::Normal, "1"),
      Segment::new(Side::Left, Class::Active, "2"),
    ]));
    snapshot.apply(Update::Tray(vec![Segment::new(
      Side::Left,
      Class::Normal,
      "tray",
    )]));
    snapshot.apply(Update::Cpu(cpu::segment(88)));
    snapshot.apply(Update::Battery(Some(power::segment(
      "󰂎 9%".into(),
      "critical",
    ))));

    assert_eq!(snapshot.audio.text, "󰖁 0%");
    assert_eq!(snapshot.workspaces[1].class, Class::Active);
    assert_eq!(snapshot.tray[0].text, "tray");
    assert_eq!(snapshot.cpu.text, "█ 88%");
    assert_eq!(snapshot.battery.as_ref().unwrap().text, "󰂎 9%");
    assert_eq!(snapshot.memory.text, "󰍛 0%");
  }
}
