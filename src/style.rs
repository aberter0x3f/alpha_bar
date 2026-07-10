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
