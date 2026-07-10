use alpha_bar::modules::audio::{default_node_name, volume_percent};

#[test]
fn pipewire_default_metadata_name() {
  assert_eq!(
    default_node_name(r#"{"name":"alsa_output.main"}"#),
    Some("alsa_output.main".into())
  );
}

#[test]
fn pipewire_linear_volume_percent() {
  assert_eq!(volume_percent(&[0.063997, 0.063997]), 6);
  assert_eq!(volume_percent(&[0.4, 0.6]), 50);
  assert_eq!(volume_percent(&[2.0]), 100);
}
