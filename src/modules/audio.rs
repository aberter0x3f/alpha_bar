use pipewire::spa::{
  self,
  pod::{Pod, deserialize::PodDeserializer},
  utils::Id,
};

pub fn default_node_name(value: &str) -> Option<String> {
  serde_json::from_str::<serde_json::Value>(value)
    .ok()
    .and_then(|json| {
      json
        .get("name")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
    })
    .or_else(|| Some(value.to_owned()).filter(|value| !value.is_empty()))
}

pub fn volume_percent(values: &[f32]) -> u8 {
  if values.is_empty() {
    return 0;
  }
  let volume = values.iter().copied().sum::<f32>() / values.len() as f32;
  ((volume * 100.0).round() as u8).min(100)
}

pub fn pipewire_props(param: &Pod) -> Option<(u8, bool)> {
  let object = param.as_object().ok()?;
  let muted = object
    .find_prop(Id(spa::sys::SPA_PROP_mute))
    .and_then(|prop| prop.value().get_bool().ok())
    .unwrap_or(false);
  let volumes = object
    .find_prop(Id(spa::sys::SPA_PROP_channelVolumes))
    .and_then(|prop| pipewire_float_array(prop.value()))?;
  Some((volume_percent(&volumes), muted))
}

fn pipewire_float_array(pod: &Pod) -> Option<Vec<f32>> {
  let (_, values) = PodDeserializer::deserialize_from::<Vec<f32>>(pod.as_bytes()).ok()?;
  Some(values)
}

pub fn text(volume: u8, muted: bool) -> String {
  if muted {
    format!("󰖁 {volume}%")
  } else if volume < 34 {
    format!("󰕿 {volume}%")
  } else if volume < 67 {
    format!("󰖀 {volume}%")
  } else {
    format!("󰕾 {volume}%")
  }
}
