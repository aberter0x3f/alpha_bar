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

pub struct BarConfig {
  pub height: u32,
}

pub struct StyleConfig {
  pub font: &'static str,
  pub background: &'static str,
  pub foreground: &'static str,
  pub border_normal: &'static str,
  pub border_active: &'static str,
  pub border_warning: &'static str,
  pub border_critical: &'static str,
}

pub const BAR: BarConfig = BarConfig { height: 32 };

pub const STYLE: StyleConfig = StyleConfig {
  font: "SarasaMonoSC Nerd Font",
  background: "#3b4252",
  foreground: "#eceff4",
  border_normal: "#4c566a",
  border_active: "#8fbcbb",
  border_warning: "#d08770",
  border_critical: "#bf616a",
};
