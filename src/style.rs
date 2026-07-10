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

use crate::{config::STYLE, state::Class};

pub fn underline(class: Class) -> &'static str {
  match class {
    Class::Active | Class::Charging | Class::Plugged => STYLE.border_active,
    Class::Warning => STYLE.border_warning,
    Class::Critical | Class::Muted | Class::Disconnected => STYLE.border_critical,
    Class::Normal => STYLE.border_normal,
  }
}

pub(crate) fn argb(hex: &str) -> u32 {
  let Some(raw) = hex.strip_prefix('#') else {
    return 0xff000000;
  };
  if raw.len() != 6 {
    return 0xff000000;
  }
  u32::from_str_radix(raw, 16).map_or(0xff000000, |rgb| 0xff000000 | rgb)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hex_argb() {
    assert_eq!(argb("#3b4252"), 0xff3b4252);
    assert_eq!(argb("bad"), 0xff000000);
  }
}
