use std::{
  num::NonZeroU32,
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
  state::{Click, Segment},
  ui::input::click_at,
  ui::render::render_segments_scaled,
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
  layer.set_margin(BAR.height as i32, 0, 0, 0);
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
    scale: 1,
    segments: segments_from_state(&state),
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
      bar.draw();
      queue.flush()?;
    }
    thread::sleep(Duration::from_millis(100));
  }
  Ok(())
}

fn segments_from_state(state: &RuntimeState) -> Vec<Segment> {
  build_segments(state.snapshot())
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
  scale: u32,
  segments: Vec<Segment>,
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
      if let Some(Click::Tray(address)) =
        click_at(self.width as usize, event.position.0, &self.segments)
      {
        let _ = self.cmd_tx.send(RuntimeCommand::TrayActivate {
          address,
          context_menu,
          x: event.position.0.round() as i32,
          y: event.position.1.round() as i32,
        });
      }
    }
  }
}

impl BarLayer {
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
    let pixels = render_segments_scaled(
      self.width as usize,
      self.height as usize,
      self.scale,
      &self.segments,
    );
    canvas.copy_from_slice(&pixels);
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
