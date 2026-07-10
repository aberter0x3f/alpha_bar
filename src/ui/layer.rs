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
    keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
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
  protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

use crate::{
  app::build_segments,
  config::BAR,
  runtime::{RuntimeCommand, Update},
  state::{Click, Snapshot, TrayMenuHit, TrayMenuPopup},
  ui::input::click_target_with_origin,
  ui::layout::menu_hit,
  ui::render::{render_menu_scaled, render_ui_scaled},
};

pub fn run(
  snapshot: Snapshot,
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

  let menu = snapshot.tray_menu.clone();
  let mut bar = BarLayer {
    registry: RegistryState::new(&globals),
    seat: SeatState::new(&globals, &qh),
    output: OutputState::new(&globals, &qh),
    shm,
    pool,
    compositor,
    layer_shell,
    layer,
    menu_layer: None,
    pointer: None,
    keyboard: None,
    width: 1,
    height: BAR.height,
    menu_open: false,
    scale: 1,
    snapshot,
    menu,
    closed: false,
    dirty: false,
    cmd_tx,
  };
  let _ = bar.cmd_tx.send(RuntimeCommand::TrayScale(bar.scale));

  queue.blocking_dispatch(&mut bar)?;
  if bar.dirty {
    bar.draw();
    queue.flush()?;
  }
  queue.flush()?;
  while !bar.closed {
    queue.dispatch_pending(&mut bar)?;
    let dirty = drain_updates(&mut bar.snapshot, &rx);
    if bar.redraw_dirty(&qh, dirty) {
      queue.flush()?;
    } else if bar.menu_layer.as_ref().is_some_and(|layer| layer.dirty) {
      bar.draw_menu();
      queue.flush()?;
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

fn drain_updates(snapshot: &mut Snapshot, rx: &Receiver<Update>) -> bool {
  let mut dirty = false;
  while let Ok(update) = rx.try_recv() {
    snapshot.apply(update);
    dirty = true;
  }
  dirty
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
  compositor: CompositorState,
  layer_shell: LayerShell,
  layer: LayerSurface,
  menu_layer: Option<MenuLayer>,
  pointer: Option<wl_pointer::WlPointer>,
  keyboard: Option<wl_keyboard::WlKeyboard>,
  width: u32,
  height: u32,
  menu_open: bool,
  scale: u32,
  snapshot: Snapshot,
  menu: Option<TrayMenuPopup>,
  closed: bool,
  dirty: bool,
  cmd_tx: Sender<RuntimeCommand>,
}

struct MenuLayer {
  layer: LayerSurface,
  width: u32,
  height: u32,
  dirty: bool,
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
    } else if capability == Capability::Keyboard && self.keyboard.is_none() {
      self.keyboard = self.seat.get_keyboard(qh, &seat, None).ok();
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
    } else if capability == Capability::Keyboard
      && let Some(keyboard) = self.keyboard.take()
    {
      keyboard.release();
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
      if self
        .menu_layer
        .as_ref()
        .is_some_and(|menu_layer| &event.surface == menu_layer.layer.wl_surface())
      {
        let PointerEventKind::Press { .. } = event.kind else {
          continue;
        };
        self.handle_menu_press(event.position.0, event.position.1);
        continue;
      }
      if &event.surface != self.layer.wl_surface() {
        continue;
      }
      let PointerEventKind::Press { button, .. } = event.kind else {
        continue;
      };
      self.handle_bar_press(button, event.position.0, event.position.1);
    }
  }
}

impl BarLayer {
  fn redraw_dirty(&mut self, qh: &QueueHandle<Self>, dirty: bool) -> bool {
    if !(dirty || self.dirty) {
      return false;
    }
    self.menu = merge_menu(self.menu.take(), self.snapshot.tray_menu.clone());
    let created_menu_layer = self.sync_menu_layer(qh, self.snapshot.tray_menu.is_some());
    if !created_menu_layer && let Some(menu_layer) = &mut self.menu_layer {
      menu_layer.dirty = true;
    }
    self.draw();
    self.draw_menu();
    true
  }

  fn sync_menu_layer(&mut self, qh: &QueueHandle<Self>, open: bool) -> bool {
    self.menu_open = open;
    if !open {
      self.menu_layer = None;
      return false;
    }
    if self.menu_layer.is_some() {
      return false;
    }
    let surface = self.compositor.create_surface(qh);
    let layer = self.layer_shell.create_layer_surface(
      qh,
      surface,
      Layer::Overlay,
      Some("alpha_bar_menu"),
      None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_size(0, 0);
    layer.set_exclusive_zone(0);
    layer.commit();
    self.menu_layer = Some(MenuLayer {
      layer,
      width: 1,
      height: 1,
      dirty: false,
    });
    true
  }

  fn handle_bar_press(&mut self, button: u32, x: f64, y: f64) {
    let context_menu = match button {
      BTN_LEFT => false,
      BTN_RIGHT => true,
      _ => return,
    };
    match click_target_with_origin(
      self.width as usize,
      BAR.height as usize,
      x,
      y,
      &build_segments(&self.snapshot),
    ) {
      Some((Click::Tray(address), menu_x)) => {
        let _ = self.cmd_tx.send(RuntimeCommand::TrayActivate {
          address,
          context_menu,
          x: x.round() as i32,
          y: y.round() as i32,
          menu_x: menu_x.round() as i32,
        });
      }
      None if self.menu_open => {
        let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuClose);
      }
      None => {}
    }
  }

  fn handle_menu_press(&mut self, x: f64, y: f64) {
    let Some(menu) = self.menu.as_mut() else {
      let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuClose);
      return;
    };
    match menu_hit(menu, x, y) {
      TrayMenuHit::Activate { id } => {
        let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuItem {
          address: menu.address.clone(),
          path: menu.path.clone(),
          id,
        });
      }
      TrayMenuHit::OpenSubmenu { path } => {
        menu.open_path = path;
        if let Some(menu_layer) = &mut self.menu_layer {
          menu_layer.dirty = true;
        }
      }
      TrayMenuHit::Outside => {
        let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuClose);
      }
    }
  }

  fn draw(&mut self) {
    self.dirty = false;
    let pixels = render_ui_scaled(
      self.width as usize,
      self.height as usize,
      self.scale,
      &build_segments(&self.snapshot),
      None,
    );
    paint_surface(
      &mut self.pool,
      &self.layer,
      self.width,
      self.height,
      self.scale,
      &pixels,
    );
  }

  fn draw_menu(&mut self) {
    let (Some(menu), Some(menu_layer)) = (&self.menu, &mut self.menu_layer) else {
      return;
    };
    if !menu_layer.dirty {
      return;
    }
    menu_layer.dirty = false;
    let pixels = render_menu_scaled(
      menu_layer.width as usize,
      menu_layer.height as usize,
      self.scale,
      menu,
    );
    paint_surface(
      &mut self.pool,
      &menu_layer.layer,
      menu_layer.width,
      menu_layer.height,
      self.scale,
      &pixels,
    );
  }
}

fn paint_surface(
  pool: &mut SlotPool,
  layer: &LayerSurface,
  width: u32,
  height: u32,
  scale: u32,
  pixels: &[u8],
) {
  let buffer_width = width * scale;
  let buffer_height = height * scale;
  let stride = buffer_width as i32 * 4;
  let (buffer, canvas) = pool
    .create_buffer(
      buffer_width as i32,
      buffer_height as i32,
      stride,
      wl_shm::Format::Argb8888,
    )
    .expect("create wayland buffer");
  copy_pixels(canvas, pixels);
  layer.wl_surface().set_buffer_scale(scale as i32);
  layer
    .wl_surface()
    .damage_buffer(0, 0, buffer_width as i32, buffer_height as i32);
  buffer
    .attach_to(layer.wl_surface())
    .expect("attach wayland buffer");
  layer.commit();
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
      let _ = self.cmd_tx.send(RuntimeCommand::TrayScale(scale));
      if let Some(menu_layer) = &mut self.menu_layer {
        menu_layer.dirty = true;
      }
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
    layer: &LayerSurface,
    configure: LayerSurfaceConfigure,
    _: u32,
  ) {
    if layer == &self.layer {
      self.width = NonZeroU32::new(configure.new_size.0).map_or(self.width, NonZeroU32::get);
      self.height = NonZeroU32::new(configure.new_size.1).map_or(BAR.height, NonZeroU32::get);
      self.dirty = true;
    } else if let Some(menu_layer) = &mut self.menu_layer
      && layer == &menu_layer.layer
    {
      menu_layer.width = NonZeroU32::new(configure.new_size.0).map_or(self.width, NonZeroU32::get);
      menu_layer.height =
        NonZeroU32::new(configure.new_size.1).map_or(self.height, NonZeroU32::get);
      menu_layer.dirty = true;
    }
  }
}

impl ShmHandler for BarLayer {
  fn shm_state(&mut self) -> &mut Shm {
    &mut self.shm
  }
}

impl KeyboardHandler for BarLayer {
  fn enter(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_keyboard::WlKeyboard,
    _: &wl_surface::WlSurface,
    _: u32,
    _: &[u32],
    _: &[Keysym],
  ) {
  }

  fn leave(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_keyboard::WlKeyboard,
    _: &wl_surface::WlSurface,
    _: u32,
  ) {
  }

  fn press_key(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_keyboard::WlKeyboard,
    _: u32,
    event: KeyEvent,
  ) {
    if self.menu_open && event.keysym == Keysym::Escape {
      let _ = self.cmd_tx.send(RuntimeCommand::TrayMenuClose);
    }
  }

  fn repeat_key(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_keyboard::WlKeyboard,
    _: u32,
    _: KeyEvent,
  ) {
  }

  fn release_key(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_keyboard::WlKeyboard,
    _: u32,
    _: KeyEvent,
  ) {
  }

  fn update_modifiers(
    &mut self,
    _: &Connection,
    _: &QueueHandle<Self>,
    _: &wl_keyboard::WlKeyboard,
    _: u32,
    _: Modifiers,
    _: RawModifiers,
    _: u32,
  ) {
  }
}

smithay_client_toolkit::delegate_compositor!(BarLayer);
smithay_client_toolkit::delegate_layer!(BarLayer);
smithay_client_toolkit::delegate_output!(BarLayer);
smithay_client_toolkit::delegate_registry!(BarLayer);
smithay_client_toolkit::delegate_seat!(BarLayer);
smithay_client_toolkit::delegate_keyboard!(BarLayer);
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
