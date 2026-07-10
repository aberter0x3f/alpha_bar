use std::{
  cell::RefCell, collections::HashMap, rc::Rc, sync::mpsc::Sender, thread, time::Duration,
};

use pipewire::{
  self as pw,
  spa::{
    self,
    pod::{Pod, deserialize::PodDeserializer},
    utils::Id,
  },
};

use crate::{
  runtime::Update,
  state::{Class, Segment, Side},
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

fn volume_percent(values: &[f32]) -> u8 {
  if values.is_empty() {
    return 0;
  }
  let volume = values.iter().copied().sum::<f32>() / values.len() as f32;
  ((volume.cbrt() * 100.0).round() as u8).min(100)
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

pub fn segment(text: String) -> Segment {
  let class = if text.starts_with('󰖁') {
    Class::Muted
  } else {
    Class::Normal
  };
  Segment::new(Side::Right, class, text)
}

pub fn spawn(tx: Sender<Update>) {
  thread::spawn(move || {
    loop {
      if let Err(err) = run(tx.clone()) {
        eprintln!("alpha_bar: PipeWire audio monitor failed: {err}");
      }
      thread::sleep(Duration::from_secs(1));
    }
  });
}

fn run(tx: Sender<Update>) -> Result<(), pw::Error> {
  use pw::{
    metadata::Metadata,
    node::Node,
    proxy::{Listener, ProxyT},
    registry::GlobalObject,
    spa::{self, param::ParamType},
    types::ObjectType,
  };

  pw::init();
  let main_loop = pw::main_loop::MainLoopRc::new(None)?;
  let context = pw::context::ContextRc::new(&main_loop, None)?;
  let core = context.connect_rc(None)?;
  let registry = core.get_registry_rc()?;
  let registry_weak = registry.downgrade();

  struct AudioState {
    tx: Sender<Update>,
    default_sink: Option<String>,
    node_id_by_name: HashMap<String, u32>,
    node_name_by_id: HashMap<u32, String>,
    nodes: HashMap<u32, Node>,
    metadata: HashMap<u32, Metadata>,
    listeners: HashMap<u32, Vec<Box<dyn Listener>>>,
    registry_listener: Option<pw::registry::Listener>,
  }

  impl AudioState {
    fn new(tx: Sender<Update>) -> Self {
      Self {
        tx,
        default_sink: None,
        node_id_by_name: HashMap::new(),
        node_name_by_id: HashMap::new(),
        nodes: HashMap::new(),
        metadata: HashMap::new(),
        listeners: HashMap::new(),
        registry_listener: None,
      }
    }

    fn remove(&mut self, id: u32) {
      let removed_default = self
        .node_name_by_id
        .get(&id)
        .is_some_and(|name| self.default_sink.as_deref() == Some(name.as_str()));
      if let Some(name) = self.node_name_by_id.remove(&id) {
        self.node_id_by_name.remove(&name);
      }
      self.nodes.remove(&id);
      self.metadata.remove(&id);
      self.listeners.remove(&id);
      if removed_default {
        self.clear_audio();
      }
    }

    fn clear_audio(&self) {
      let _ = self.tx.send(Update::Audio(segment(text(0, true))));
    }

    fn refresh_default(&self) {
      let Some(name) = self.default_sink.as_deref() else {
        return;
      };
      let Some(id) = self.node_id_by_name.get(name) else {
        return;
      };
      if let Some(node) = self.nodes.get(id) {
        node.enum_params(0, Some(ParamType::Props), 0, u32::MAX);
      }
    }
  }

  fn bind_node(
    registry: &pw::registry::Registry,
    global: &GlobalObject<&spa::utils::dict::DictRef>,
    state: Rc<RefCell<AudioState>>,
  ) {
    let Some(props) = global.props.as_ref() else {
      return;
    };
    if props.get("media.class") != Some("Audio/Sink") {
      return;
    }
    let Some(name) = props.get("node.name").map(str::to_owned) else {
      return;
    };
    let Ok(node) = registry.bind::<Node, _>(global) else {
      return;
    };
    let node_id = global.id;
    {
      let mut state = state.borrow_mut();
      state.node_id_by_name.insert(name.clone(), node_id);
      state.node_name_by_id.insert(node_id, name);
    }
    let listener = node
      .add_listener_local()
      .param({
        let state = Rc::clone(&state);
        move |_seq, id, _index, _next, param| {
          if id != ParamType::Props {
            return;
          }
          let Some(param) = param else {
            return;
          };
          let is_default = {
            let state = state.borrow();
            state
              .node_name_by_id
              .get(&node_id)
              .is_some_and(|name| state.default_sink.as_deref() == Some(name.as_str()))
          };
          if !is_default {
            return;
          }
          if let Some((volume, muted)) = pipewire_props(param) {
            let _ = state
              .borrow()
              .tx
              .send(Update::Audio(segment(text(volume, muted))));
          }
        }
      })
      .register();
    node.subscribe_params(&[ParamType::Props]);
    node.enum_params(0, Some(ParamType::Props), 0, u32::MAX);
    let proxy_listener = node
      .upcast_ref()
      .add_listener_local()
      .removed({
        let state = Rc::clone(&state);
        move || state.borrow_mut().remove(node_id)
      })
      .register();
    let mut state = state.borrow_mut();
    state.nodes.insert(node_id, node);
    state
      .listeners
      .entry(node_id)
      .or_default()
      .push(Box::new(listener));
    state
      .listeners
      .entry(node_id)
      .or_default()
      .push(Box::new(proxy_listener));
    state.refresh_default();
  }

  fn bind_metadata(
    registry: &pw::registry::Registry,
    global: &GlobalObject<&spa::utils::dict::DictRef>,
    state: Rc<RefCell<AudioState>>,
  ) {
    let Some(props) = global.props.as_ref() else {
      return;
    };
    if props.get("metadata.name") != Some("default") {
      return;
    }
    let Ok(metadata) = registry.bind::<Metadata, _>(global) else {
      return;
    };
    let metadata_id = global.id;
    let listener = metadata
      .add_listener_local()
      .property({
        let state = Rc::clone(&state);
        move |_subject, key, _type, value| {
          if key != Some("default.audio.sink") {
            return 0;
          }
          let mut state = state.borrow_mut();
          state.default_sink = value.and_then(default_node_name);
          if state.default_sink.is_none() {
            state.clear_audio();
          }
          state.refresh_default();
          0
        }
      })
      .register();
    let proxy_listener = metadata
      .upcast_ref()
      .add_listener_local()
      .removed({
        let state = Rc::clone(&state);
        move || state.borrow_mut().remove(metadata_id)
      })
      .register();
    let mut state = state.borrow_mut();
    state.metadata.insert(metadata_id, metadata);
    state
      .listeners
      .entry(metadata_id)
      .or_default()
      .push(Box::new(listener));
    state
      .listeners
      .entry(metadata_id)
      .or_default()
      .push(Box::new(proxy_listener));
  }

  let state = Rc::new(RefCell::new(AudioState::new(tx)));
  let registry_listener = registry
    .add_listener_local()
    .global({
      let state = Rc::clone(&state);
      move |global| {
        let Some(registry) = registry_weak.upgrade() else {
          return;
        };
        if global.type_ == ObjectType::Node {
          bind_node(&registry, global, Rc::clone(&state));
        } else if global.type_ == ObjectType::Metadata {
          bind_metadata(&registry, global, Rc::clone(&state));
        }
      }
    })
    .global_remove({
      let state = Rc::clone(&state);
      move |id| state.borrow_mut().remove(id)
    })
    .register();
  state.borrow_mut().registry_listener = Some(registry_listener);

  main_loop.run();
  unsafe { pw::deinit() };
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_name() {
    assert_eq!(
      default_node_name(r#"{"name":"alsa_output.main"}"#),
      Some("alsa_output.main".into())
    );
  }

  #[test]
  fn cubic_volume() {
    assert_eq!(volume_percent(&[0.064, 0.064]), 40);
    assert_eq!(volume_percent(&[0.216, 0.216]), 60);
    assert_eq!(volume_percent(&[0.512, 0.512]), 80);
    assert_eq!(volume_percent(&[2.0]), 100);
  }
}
