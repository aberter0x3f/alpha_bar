use std::{
  io,
  num::NonZeroU32,
  os::fd::{AsRawFd, BorrowedFd},
  sync::mpsc::{Receiver, Sender},
  thread,
  time::Duration,
};

use smithay_client_toolkit::{
  compositor::{CompositorHandler, CompositorState},
  output::{OutputHandler, OutputState},
  registry::{ProvidesRegistryState, RegistryState},
  registry_handlers,
  seat::{
    Capability, SeatHandler, SeatState,
    pointer::{BTN_LEFT, BTN_RIGHT, PointerEvent, PointerEventKind, PointerHandler},
  },
  shell::{
    WaylandSurface,
    wlr_layer::{
      Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
      LayerSurfaceConfigure,
    },
  },
  shm::{Shm, ShmHandler, slot::SlotPool},
};
use wayland_client::{
  Connection, QueueHandle,
  globals::registry_queue_init,
  protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

use crate::{
  app::build_segments,
  config::BAR,
  runtime::{RuntimeCommand, RuntimeState, Update},
  state::{Click, Segment, TrayMenuHit, TrayMenuPopup},
  ui::input::click_target,
  ui::render::render_ui_scaled,
};

pub fn run(
  mut state: RuntimeState,
  rx: Receiver<Update>,
  cmd_tx: Sender<RuntimeCommand>,
) -> Result<(), Box<dyn std::error::Error>> {
  let conn = Connection::connect_to_env()?;
  let (globals, mut queue) = registry_queue_init(&conn)?;
  let qh = queue.handle();
  let compositor = CompositorState::bind(&globals, &qh)?;
  let layer_shell = LayerShell::bind(&globals, &qh)?;
  let shm = Shm::bind(&globals, &qh)?;
  let pool = SlotPool::new(BAR.height as usize * 4, &shm)?;

  let surface = compositor.create_surface(&qh);
  let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("alpha_bar"), None);
  layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
  layer.set_keyboard_interactivity(KeyboardInteractivity::None);
  layer.set_size(0, BAR.height);
  layer.set_exclusive_zone(BAR.height as i32);
  layer.commit();

  let mut bar = BarLayer {
    registry: RegistryState::new(&globals),
    seat: SeatState::new(&globals, &qh),
    output: OutputState::new(&globals, &qh),
    shm,
    pool,
    layer,
    pointer: None,
    width: 1,
    height: BAR.height,
    menu_open: false,
    scale: 1,
    segments: segments_from_state(&state),
    menu: state.snapshot().tray_menu.clone(),
    closed: false,
    dirty: false,
    cmd_tx,
  };

  queue.blocking_dispatch(&mut bar)?;
  if bar.dirty {
    bar.draw();
    queue.flush()?;
  }
  queue.flush()?;
  while !bar.closed {
    queue.dispatch_pending(&mut bar)?;
    let mut dirty = false;
    while let Ok(update) = rx.try_recv() {
      state.apply(update);
      dirty = true;
    }
    if dirty || bar.dirty {
      bar.segments = segments_from_state(&state);
      bar.menu = merge_menu(bar.menu.take(), state.snapshot().tray_menu.clone());
      if bar.set_menu_open(state.snapshot().tray_menu.is_some()) {
        queue.flush()?;
      } else {
        bar.draw();
        queue.flush()?;
      }
    }
    queue.flush()?;
    if let Some(read) = queue.prepare_read() {
      if fd_readable(read.connection_fd(), 100)? {
        read.read()?;
      }
    } else {
      thread::sleep(Duration::from_millis(10));
    }
  }
  Ok(())
}

fn fd_readable(fd: BorrowedFd<'_>, timeout_ms: i32) -> io::Result<bool> {
  let mut poll_fd = libc::pollfd {
    fd: fd.as_raw_fd(),
    events: libc::POLLIN,
    revents: 0,
  };
  loop {
    let ready = unsafe { libc::poll(&mut poll_fd, 1, timeout_ms) };
    if ready >= 0 {
      return Ok(ready > 0 && poll_fd.revents & libc::POLLIN != 0);
    }
    let error = io::Error::last_os_error();
    if error.kind() != io::ErrorKind::Interrupted {
      return Err(error);
    }
  }
}

fn segments_from_state(state: &RuntimeState) -> Vec<Segment> {
  build_segments(state.snapshot())
}

fn merge_menu(
  current: Option<TrayMenuPopup>,
  mut next: Option<TrayMenuPopup>,
) -> Option<TrayMenuPopup> {
  if let (Some(current), Some(next_menu)) = (&current, &mut next)
    && current.address == next_menu.address
    && current.path == next_menu.path
  {
    next_menu.open_path.clone_from(&current.open_path);
  }
  next
}

struct BarLayer {
  registry: RegistryState,
  seat: SeatState,
  output: OutputState,
  shm: Shm,
  pool: SlotPool,
  layer: LayerSurface,
  pointer: Option<wl_pointer::WlPointer>,
  width: u32,
  height: u32,
  menu_open: bool,
  scale: u32,
  segments: Vec<Segment>,
  menu: Option<TrayMenuPopup>,
  closed: bool,
  dirty: bool,
  cmd_tx: Sender<RuntimeCommand>,
}

impl SeatHandler for BarLayer {
  fn seat_state(&mut self) -> &mut SeatState {
    &mut self.seat
  }

  fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

  fn new_capability(
    &mut self,
    _: &Connection,
    qh: &QueueHandle<Self>,
    seat: wl_seat::WlSeat,
    capability: Capability,
  ) {
    if capability == Capability::Pointer && self.pointer.is_none() {
      self.pointer = self.seat.get_pointer(qh, &seat).ok();
    }
  }

  fn remove_capability(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: wl_seat::WlSeat,
    capability: Capability,
  ) {
    if capability == Capability::Pointer
      && let Some(pointer) = self.pointer.take()
    {
      pointer.release();
    }
  }

  fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for BarLayer {
  fn pointer_frame(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_pointer::WlPointer,
    events: &[PointerEvent],
  ) {
    for event in events {
      if &event.surface != self.layer.wl_surface() {
        continue;
      }
      let PointerEventKind::Press { button, .. } = event.kind else {
        continue;
      };
      let context_menu = match button {
        BTN_LEFT => false,
        BTN_RIGHT => true,
        _ => continue,
      };
      if let Some(menu) = self.menu.as_mut() {
        match menu.hit(event.position.0, event.position.1) {
          TrayMenuHit::Activate { id } => {
            let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuItem {
              address: menu.address.clone(),
              path: menu.path.clone(),
              id,
            });
            continue;
          }
          TrayMenuHit::OpenSubmenu { path } => {
            menu.open_path = path;
            self.draw();
            continue;
          }
          TrayMenuHit::Outside => {
            let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuClose);
            continue;
          }
        }
      }
      match click_target(
        self.width as usize,
        BAR.height as usize,
        event.position.0,
        event.position.1,
        &self.segments,
        self.menu.as_ref(),
      ) {
        Some(Click::Tray(address)) => {
          let _ = self.cmd_tx.send(RuntimeCommand::TrayActivate {
            address,
            context_menu,
            x: event.position.0.round() as i32,
            y: event.position.1.round() as i32,
          });
        }
        None => {}
      }
    }
  }
}

impl BarLayer {
  fn set_menu_open(&mut self, open: bool) -> bool {
    if self.menu_open != open {
      self.menu_open = open;
      if open {
        self
          .layer
          .set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        self.layer.set_size(0, 0);
      } else {
        self
          .layer
          .set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
        self.layer.set_size(0, BAR.height);
      }
      self.layer.set_exclusive_zone(BAR.height as i32);
      self.layer.commit();
      return true;
    }
    false
  }

  fn draw(&mut self) {
    self.dirty = false;
    let buffer_width = self.width * self.scale;
    let buffer_height = self.height * self.scale;
    let stride = buffer_width as i32 * 4;
    let (buffer, canvas) = self
      .pool
      .create_buffer(
        buffer_width as i32,
        buffer_height as i32,
        stride,
        wl_shm::Format::Argb8888,
      )
      .expect("create wayland buffer");
    let pixels = render_ui_scaled(
      self.width as usize,
      self.height as usize,
      self.scale,
      &self.segments,
      self.menu.as_ref(),
    );
    copy_pixels(canvas, &pixels);
    self.layer.wl_surface().set_buffer_scale(self.scale as i32);
    self
      .layer
      .wl_surface()
      .damage_buffer(0, 0, buffer_width as i32, buffer_height as i32);
    buffer
      .attach_to(self.layer.wl_surface())
      .expect("attach wayland buffer");
    self.layer.commit();
  }
}

fn copy_pixels(canvas: &mut [u8], pixels: &[u8]) {
  canvas.fill(0);
  let len = canvas.len().min(pixels.len());
  canvas[..len].copy_from_slice(&pixels[..len]);
}

impl CompositorHandler for BarLayer {
  fn scale_factor_changed(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    scale: i32,
  ) {
    let scale = u32::try_from(scale).unwrap_or(1).max(1);
    if self.scale != scale {
      self.scale = scale;
      self.dirty = true;
    }
  }

  fn transform_changed(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    _: wl_output::Transform,
  ) {
  }

  fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

  fn surface_enter(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    _: &wl_output::WlOutput,
  ) {
  }

  fn surface_leave(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_surface::WlSurface,
    _: &wl_output::WlOutput,
  ) {
  }
}

impl OutputHandler for BarLayer {
  fn output_state(&mut self) -> &mut OutputState {
    &mut self.output
  }

  fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

  fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

  fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for BarLayer {
  fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
    self.closed = true;
  }

  fn configure(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &LayerSurface,
    configure: LayerSurfaceConfigure,
    _: u32,
  ) {
    self.width = NonZeroU32::new(configure.new_size.0).map_or(self.width, NonZeroU32::get);
    self.height = NonZeroU32::new(configure.new_size.1).map_or(BAR.height, NonZeroU32::get);
    self.dirty = true;
  }
}

impl ShmHandler for BarLayer {
  fn shm_state(&mut self) -> &mut Shm {
    &mut self.shm
  }
}

smithay_client_toolkit::delegate_compositor!(BarLayer);
smithay_client_toolkit::delegate_layer!(BarLayer);
smithay_client_toolkit::delegate_output!(BarLayer);
smithay_client_toolkit::delegate_registry!(BarLayer);
smithay_client_toolkit::delegate_seat!(BarLayer);
smithay_client_toolkit::delegate_pointer!(BarLayer);
smithay_client_toolkit::delegate_shm!(BarLayer);

impl ProvidesRegistryState for BarLayer {
  fn registry(&mut self) -> &mut RegistryState {
    &mut self.registry
  }
  registry_handlers![OutputState, SeatState];
}

#[cfg(test)]
mod tests {
  use crate::state::{TrayMenuEntry, TrayMenuPopup};
  use std::{io::Write, os::fd::AsFd, os::unix::net::UnixStream};

  fn menu(address: &str) -> TrayMenuPopup {
    TrayMenuPopup {
      address: address.into(),
      path: "/menu".into(),
      x: 0.0,
      y: 32.0,
      entries: vec![TrayMenuEntry {
        id: 1,
        label: "Menu".into(),
        enabled: true,
        separator: false,
        icon: None,
        children: Vec::new(),
      }],
      open_path: Vec::new(),
    }
  }

  #[test]
  fn fd_ready() {
    let (mut write, read) = UnixStream::pair().unwrap();
    assert!(!super::fd_readable(read.as_fd(), 0).unwrap());
    write.write_all(&[1]).unwrap();
    assert!(super::fd_readable(read.as_fd(), 100).unwrap());
  }

  #[test]
  fn copy_short_pixels() {
    let mut canvas = [9; 6];
    super::copy_pixels(&mut canvas, &[1, 2, 3, 4]);
    assert_eq!(canvas, [1, 2, 3, 4, 0, 0]);
  }

  #[test]
  fn keep_submenu_path() {
    let mut current = menu(":1.1/StatusNotifierItem");
    current.open_path = vec![0];
    let merged = super::merge_menu(Some(current), Some(menu(":1.1/StatusNotifierItem"))).unwrap();
    assert_eq!(merged.open_path, vec![0]);
  }
}
