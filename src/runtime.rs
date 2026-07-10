use std::{
  cell::RefCell,
  collections::HashMap,
  fs,
  io::{BufRead, BufReader},
  path::PathBuf,
  rc::Rc,
  sync::mpsc::{Receiver, Sender},
  thread,
  time::{Duration, Instant},
};

use pipewire as pw;
use system_tray::client::ActivateRequest;

use crate::{
  app::Snapshot,
  clock::clock_text,
  modules::{audio, network, tray},
  net_speed::{format_rates, rates_since},
  niri::{NiriState, open_event_stream, request_workspaces},
  power,
  proc_parse::{cpu_usage, mem_usage},
  state::{Class, Click, Segment, Side},
};

pub enum Update {
  Workspaces(Vec<Segment>),
  Tray(Vec<Segment>),
  Network(String),
  NetSpeed(String),
  Audio(String),
  Cpu(u8),
  Memory(u8),
  Temperature(i64),
  Battery(String, Class),
  Clock(String),
}

pub enum RuntimeCommand {
  TrayActivate {
    address: String,
    context_menu: bool,
    x: i32,
    y: i32,
  },
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
        network: "󰲛 Disconnected".into(),
        net_speed: format_rates(0, 0, 1_000_000),
        audio: "󰖁 0%".into(),
        cpu: 0,
        memory: 0,
        temperature: 0,
        battery: " 100%".into(),
        battery_class: Class::Plugged,
        clock: clock_text(0, 0),
      },
    }
  }
}

impl RuntimeState {
  pub fn apply(&mut self, update: Update) {
    match update {
      Update::Workspaces(v) => self.snapshot.workspaces = v,
      Update::Tray(v) => self.snapshot.tray = v,
      Update::Network(v) => self.snapshot.network = v,
      Update::NetSpeed(v) => self.snapshot.net_speed = v,
      Update::Audio(v) => self.snapshot.audio = v,
      Update::Cpu(v) => self.snapshot.cpu = v,
      Update::Memory(v) => self.snapshot.memory = v,
      Update::Temperature(v) => self.snapshot.temperature = v,
      Update::Battery(text, class) => {
        self.snapshot.battery = text;
        self.snapshot.battery_class = class;
      }
      Update::Clock(v) => self.snapshot.clock = v,
    }
  }

  pub fn snapshot(&self) -> &Snapshot {
    &self.snapshot
  }
}

pub fn spawn_workers(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  spawn_loop(tx.clone(), Duration::from_secs(2), || {
    Update::Network(current_network_text())
  });
  spawn_audio(tx.clone());
  spawn_loop(tx.clone(), Duration::from_secs(5), || {
    Update::Memory(current_memory())
  });
  spawn_loop(tx.clone(), Duration::from_secs(5), || {
    Update::Temperature(current_temperature())
  });
  spawn_loop(tx.clone(), Duration::from_secs(5), || {
    let (text, class) = battery_text();
    Update::Battery(text, class)
  });
  spawn_loop(tx.clone(), Duration::from_secs(1), || {
    Update::Clock(current_clock())
  });
  spawn_net_speed(tx.clone());
  spawn_niri(tx.clone());
  spawn_tray(tx.clone(), cmd_rx);
  spawn_cpu(tx);
}

fn spawn_loop(
  update_tx: Sender<Update>,
  interval: Duration,
  mut read: impl FnMut() -> Update + Send + 'static,
) {
  thread::spawn(move || {
    loop {
      let _ = update_tx.send(read());
      thread::sleep(interval);
    }
  });
}

fn spawn_cpu(tx: Sender<Update>) {
  thread::spawn(move || {
    let mut old = fs::read_to_string("/proc/stat").unwrap_or_default();
    loop {
      thread::sleep(Duration::from_secs(5));
      let new = fs::read_to_string("/proc/stat").unwrap_or_default();
      let _ = tx.send(Update::Cpu(cpu_usage(&old, &new).unwrap_or(0)));
      old = new;
    }
  });
}

fn spawn_net_speed(tx: Sender<Update>) {
  thread::spawn(move || {
    let mut iface = None;
    let mut old = None;
    let mut then = Instant::now();
    loop {
      let route = fs::read_to_string("/proc/net/route").unwrap_or_default();
      let current_iface = network::default_iface(&route).map(str::to_owned);
      if current_iface != iface {
        iface = current_iface;
        old = None;
        then = Instant::now();
      }
      if let Some(name) = iface.as_deref()
        && let Ok(dev) = fs::read_to_string("/proc/net/dev")
        && let Some(new) = network::dev_bytes(&dev, name)
      {
        if let Some(text) = rates_since(old, new, then.elapsed().as_micros() as u64) {
          let _ = tx.send(Update::NetSpeed(text));
        }
        old = Some(new);
        then = Instant::now();
      }
      thread::sleep(Duration::from_secs(2));
    }
  });
}

fn spawn_audio(tx: Sender<Update>) {
  thread::spawn(move || {
    loop {
      if let Err(err) = run_pipewire_audio(tx.clone()) {
        eprintln!("alpha_bar: PipeWire audio monitor failed: {err}");
      }
      thread::sleep(Duration::from_secs(1));
    }
  });
}

fn run_pipewire_audio(tx: Sender<Update>) -> Result<(), pw::Error> {
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
      let _ = self.tx.send(Update::Audio(audio::text(0, true)));
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
          if let Some((volume, muted)) = audio::pipewire_props(param) {
            let _ = state
              .borrow()
              .tx
              .send(Update::Audio(audio::text(volume, muted)));
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
          state.default_sink = value.and_then(audio::default_node_name);
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

fn spawn_niri(tx: Sender<Update>) {
  thread::spawn(move || {
    let Ok(socket) = std::env::var("NIRI_SOCKET") else {
      return;
    };
    let mut state = NiriState::default();
    if let Ok(workspaces) = request_workspaces(&socket) {
      state.set_workspaces(workspaces);
      if let Some(output) = state.focused_output() {
        let _ = tx.send(Update::Workspaces(state.workspace_segments(output)));
      }
    }
    let Ok(stream) = open_event_stream(socket) else {
      return;
    };
    for line in BufReader::new(stream).lines().map_while(Result::ok) {
      let Ok(event) = serde_json::from_str(&line) else {
        continue;
      };
      state.apply(&event);
      let Some(output) = state.focused_output() else {
        continue;
      };
      let _ = tx.send(Update::Workspaces(state.workspace_segments(output)));
    }
  });
}

fn spawn_tray(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  thread::spawn(move || {
    let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .build()
    else {
      return;
    };
    rt.block_on(async move {
      let Ok(client) = system_tray::client::Client::new().await else {
        return;
      };
      let mut events = client.subscribe();
      let _ = tx.send(Update::Tray(tray_segments(&client)));
      loop {
        tokio::select! {
          event = events.recv() => {
            if event.is_err() {
              return;
            }
            let _ = tx.send(Update::Tray(tray_segments(&client)));
          }
          _ = tokio::time::sleep(Duration::from_millis(50)) => {
            while let Ok(command) = cmd_rx.try_recv() {
              match command {
                RuntimeCommand::TrayActivate { address, context_menu, x, y } => {
                  if context_menu {
                    let _ = open_context_menu(&address, x, y).await;
                  } else {
                    let _ = client.activate(ActivateRequest::Default { address, x, y }).await;
                  }
                }
              }
            }
          }
        }
      }
    });
  });
}

fn tray_segments(client: &system_tray::client::Client) -> Vec<Segment> {
  let items = client.items();
  let Ok(items) = items.lock() else {
    return Vec::new();
  };
  let xdg_data_roots = xdg_data_roots();
  let icon_theme = current_icon_theme();
  items
    .iter()
    .filter_map(|(address, (item, _))| {
      item
        .icon_name
        .as_deref()
        .and_then(|name| {
          tray::find_icon_path(
            name,
            item.icon_theme_path.as_deref().map(std::path::Path::new),
            &xdg_data_roots,
            &icon_theme,
          )
          .and_then(|path| tray::icon_from_path(&path))
        })
        .or_else(|| {
          item
            .icon_pixmap
            .as_ref()
            .and_then(|pixmaps| {
              pixmaps
                .iter()
                .max_by_key(|pixmap| pixmap.width * pixmap.height)
            })
            .and_then(|pixmap| tray::icon_from_argb(pixmap.width, pixmap.height, &pixmap.pixels))
        })
        .map(|icon| {
          Segment::icon(Side::Left, Class::Normal, icon).click(Click::Tray(address.clone()))
        })
    })
    .collect()
}

async fn open_context_menu(address: &str, x: i32, y: i32) -> zbus::Result<()> {
  let (destination, path) = parse_tray_address(address);
  let connection = zbus::Connection::session().await?;
  let proxy =
    zbus::Proxy::new(&connection, destination, path, "org.kde.StatusNotifierItem").await?;
  proxy.call_method("ContextMenu", &(x, y)).await?;
  Ok(())
}

fn parse_tray_address(address: &str) -> (&str, String) {
  address.split_once('/').map_or(
    (address, "/StatusNotifierItem".into()),
    |(destination, path)| (destination, format!("/{path}")),
  )
}

fn current_icon_theme() -> String {
  let dirs = config_dirs();
  dirs
    .iter()
    .find_map(|dir| read_icon_theme(&dir.join("gtk-3.0/settings.ini")))
    .or_else(|| {
      dirs
        .iter()
        .find_map(|dir| read_icon_theme(&dir.join("gtk-4.0/settings.ini")))
    })
    .or_else(|| {
      dirs
        .iter()
        .find_map(|dir| read_kde_icon_theme(&dir.join("kdeglobals")))
    })
    .unwrap_or_else(|| "hicolor".into())
}

fn config_dirs() -> Vec<PathBuf> {
  if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
    vec![PathBuf::from(dir)]
  } else if let Some(home) = std::env::var_os("HOME") {
    vec![PathBuf::from(home).join(".config")]
  } else {
    Vec::new()
  }
}

fn read_icon_theme(path: &std::path::Path) -> Option<String> {
  fs::read_to_string(path)
    .ok()?
    .lines()
    .map(str::trim)
    .find_map(|line| {
      line
        .strip_prefix("gtk-icon-theme-name=")
        .map(str::trim)
        .map(str::to_owned)
    })
    .filter(|name| !name.is_empty())
}

fn read_kde_icon_theme(path: &std::path::Path) -> Option<String> {
  let mut in_icons = false;
  for line in fs::read_to_string(path).ok()?.lines().map(str::trim) {
    if line.starts_with('[') && line.ends_with(']') {
      in_icons = &line[1..line.len() - 1] == "Icons";
    } else if in_icons
      && let Some(theme) = line.strip_prefix("Theme=").map(str::trim)
      && !theme.is_empty()
    {
      return Some(theme.to_owned());
    }
  }
  None
}

fn xdg_data_roots() -> Vec<PathBuf> {
  let mut roots = vec![PathBuf::from("/run/current-system/sw/share")];
  if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
    roots.push(PathBuf::from(home));
  } else if let Some(home) = std::env::var_os("HOME") {
    roots.push(PathBuf::from(home).join(".local/share"));
  }
  if let Some(dirs) = std::env::var_os("XDG_DATA_DIRS") {
    roots.extend(std::env::split_paths(&dirs));
  }
  roots
}

pub fn network_text(iface: Option<&str>, operstate: &str) -> String {
  match (iface, operstate.trim()) {
    (None, _) => "󰲛 Disconnected".into(),
    (Some(iface), "up") if iface.starts_with('w') => {
      wifi_network_text(iface, wireless_ssid(iface).as_deref())
    }
    (Some(iface), "up") => format!("󰈀 {iface}"),
    (Some(iface), _) => format!("{iface} (No IP)"),
  }
}

pub fn wifi_network_text(iface: &str, ssid: Option<&str>) -> String {
  format!("󰤨 {}", ssid.filter(|s| !s.is_empty()).unwrap_or(iface))
}

fn wireless_ssid(iface: &str) -> Option<String> {
  const SIOCGIWESSID: libc::c_ulong = 0x8B1B;
  #[repr(C)]
  struct IwPoint {
    pointer: *mut libc::c_void,
    length: u16,
    flags: u16,
  }
  #[repr(C)]
  union IwreqData {
    essid: std::mem::ManuallyDrop<IwPoint>,
  }
  #[repr(C)]
  struct Iwreq {
    name: [libc::c_char; libc::IFNAMSIZ],
    data: IwreqData,
  }

  let mut essid = [0u8; 33];
  let mut req = Iwreq {
    name: [0; libc::IFNAMSIZ],
    data: IwreqData {
      essid: std::mem::ManuallyDrop::new(IwPoint {
        pointer: essid.as_mut_ptr().cast(),
        length: 32,
        flags: 0,
      }),
    },
  };
  for (dst, src) in req.name.iter_mut().zip(iface.bytes()) {
    *dst = src as libc::c_char;
  }
  // SAFETY: ioctl writes at most `length` bytes into the valid stack buffer above.
  let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
  if fd < 0 {
    return None;
  }
  let ok = unsafe { libc::ioctl(fd, SIOCGIWESSID, &mut req) } == 0;
  let _ = unsafe { libc::close(fd) };
  if !ok {
    return None;
  }
  let len = unsafe { req.data.essid.length as usize }.min(essid.len());
  String::from_utf8(
    essid[..len]
      .iter()
      .copied()
      .take_while(|b| *b != 0)
      .collect(),
  )
  .ok()
}

fn current_network_text() -> String {
  let route = fs::read_to_string("/proc/net/route").unwrap_or_default();
  let iface = network::default_iface(&route);
  let operstate = iface
    .and_then(|iface| fs::read_to_string(format!("/sys/class/net/{iface}/operstate")).ok())
    .unwrap_or_default();
  network_text(iface, &operstate)
}

fn current_memory() -> u8 {
  fs::read_to_string("/proc/meminfo")
    .ok()
    .and_then(|s| mem_usage(&s))
    .map_or(0, |m| m.used_percent)
}

fn current_temperature() -> i64 {
  fs::read_to_string("/sys/class/thermal/thermal_zone3/temp")
    .ok()
    .and_then(|s| power::temp_c(&s))
    .unwrap_or(0)
}

fn current_clock() -> String {
  let now = chrono::Local::now();
  clock_text(
    now.format("%H").to_string().parse().unwrap_or(0),
    now.format("%M").to_string().parse().unwrap_or(0),
  )
}

fn battery_text() -> (String, Class) {
  power::system_battery(std::path::Path::new("/sys/class/power_supply")).map_or_else(
    || ("󰂑 N/A".into(), Class::Disconnected),
    |battery| (battery.text, class_from_battery(battery.class)),
  )
}

fn class_from_battery(class: &str) -> Class {
  match class {
    "warning" => Class::Warning,
    "critical" => Class::Critical,
    "charging" => Class::Charging,
    "plugged" => Class::Plugged,
    _ => Class::Normal,
  }
}
