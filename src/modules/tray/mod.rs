use std::{
  path::Path,
  sync::mpsc::{self, Receiver, Sender},
  thread,
};

use crate::{
  runtime::{RuntimeCommand, Update},
  state::{Class, Click, Segment, Side, TrayMenuPopup},
};

mod bus;
mod dbus_menu;
mod icons;
mod sni;
mod watcher;

use dbus_menu::{activate_menu_item, menu_address, pending_menu, refresh_tray_menu};
use icons::{choose_pixmap, current_icon_theme, icon_from_name, tray_icon_size, xdg_data_roots};
use sni::{TrayItem, activate, open_context_menu, tray_items};
use watcher::watch_tray;

pub fn spawn(tx: Sender<Update>, cmd_rx: Receiver<RuntimeCommand>) {
  thread::spawn(move || {
    let (pump_tx, pump_rx) = mpsc::channel();
    let cmd_tx = pump_tx.clone();
    thread::spawn(move || {
      while let Ok(command) = cmd_rx.recv() {
        if cmd_tx.send(PumpEvent::Command(command)).is_err() {
          break;
        }
      }
    });
    watch_tray(pump_tx.clone());
    let mut tray_generation = 0u64;
    let mut menu_generation = 0u64;
    let mut icon_size = tray_icon_size(1);
    let mut refresh_running = false;
    let mut refresh_pending = false;
    request_segments(
      &pump_tx,
      &mut tray_generation,
      &mut refresh_running,
      &mut refresh_pending,
      icon_size,
    );
    while let Ok(event) = pump_rx.recv() {
      match event {
        PumpEvent::Refresh => {
          request_segments(
            &pump_tx,
            &mut tray_generation,
            &mut refresh_running,
            &mut refresh_pending,
            icon_size,
          );
        }
        PumpEvent::Job(TrayJob::Segments(generation, segments)) => {
          if generation == tray_generation {
            let _ = tx.send(Update::Tray(segments));
            refresh_running = false;
            if refresh_pending {
              refresh_pending = false;
              request_segments(
                &pump_tx,
                &mut tray_generation,
                &mut refresh_running,
                &mut refresh_pending,
                icon_size,
              );
            }
          }
        }
        PumpEvent::Job(TrayJob::Menu(generation, menu)) if generation == menu_generation => {
          let _ = tx.send(Update::TrayMenu(Some(menu)));
        }
        PumpEvent::Job(TrayJob::MenuError(generation, address, error))
          if generation == menu_generation =>
        {
          eprintln!("tray menu failed for {address}: {error}");
        }
        PumpEvent::Job(_) => {}
        PumpEvent::Command(RuntimeCommand::TrayActivate {
          address,
          context_menu,
          x,
          y,
          menu_x,
        }) => {
          if context_menu && let Some((path, item_theme_path)) = menu_address(&address) {
            menu_generation += 1;
            let generation = menu_generation;
            let _ = tx.send(Update::TrayMenu(Some(pending_menu(
              address.clone(),
              path.clone(),
              menu_x,
            ))));
            let tx = pump_tx.clone();
            thread::spawn(move || {
              let result =
                refresh_tray_menu(address.clone(), path, item_theme_path, menu_x, icon_size)
                  .map(|menu| TrayJob::Menu(generation, menu))
                  .unwrap_or_else(|error| {
                    TrayJob::MenuError(generation, address, error.to_string())
                  });
              let _ = tx.send(PumpEvent::Job(result));
            });
            continue;
          }
          thread::spawn(move || {
            let result = if context_menu {
              open_context_menu(&address, x, y)
            } else {
              activate(&address, x, y)
            };
            if let Err(error) = result {
              eprintln!("tray click failed for {address}: {error}");
            }
          });
        }
        PumpEvent::Command(RuntimeCommand::TrayMenuItem { address, path, id }) => {
          thread::spawn(move || {
            if let Err(error) = activate_menu_item(&address, &path, id) {
              eprintln!("tray menu item failed for {address}: {error}");
            }
          });
          let _ = tx.send(Update::TrayMenu(None));
        }
        PumpEvent::Command(RuntimeCommand::TrayMenuClose) => {
          let _ = tx.send(Update::TrayMenu(None));
        }
        PumpEvent::Command(RuntimeCommand::TrayScale(scale)) => {
          let next = tray_icon_size(scale);
          if icon_size != next {
            icon_size = next;
            request_segments(
              &pump_tx,
              &mut tray_generation,
              &mut refresh_running,
              &mut refresh_pending,
              icon_size,
            );
          }
        }
      }
    }
  });
}

fn request_segments(
  tx: &Sender<PumpEvent>,
  generation: &mut u64,
  running: &mut bool,
  pending: &mut bool,
  icon_size: u32,
) {
  if *running {
    *pending = true;
    return;
  }
  *generation += 1;
  *running = true;
  schedule_segments(tx, *generation, icon_size);
}

enum PumpEvent {
  Refresh,
  Command(RuntimeCommand),
  Job(TrayJob),
}

enum TrayJob {
  Segments(u64, Vec<Segment>),
  Menu(u64, TrayMenuPopup),
  MenuError(u64, String, String),
}

fn schedule_segments(tx: &Sender<PumpEvent>, generation: u64, icon_size: u32) {
  let tx = tx.clone();
  thread::spawn(move || {
    let segments = segments(tray_items(), icon_size);
    let _ = tx.send(PumpEvent::Job(TrayJob::Segments(generation, segments)));
  });
}

fn segments(items: Vec<(String, TrayItem)>, icon_size: u32) -> Vec<Segment> {
  let xdg_data_roots = xdg_data_roots();
  let icon_theme = current_icon_theme();
  items
    .into_iter()
    .filter_map(|(address, item)| {
      item
        .icon_name
        .as_deref()
        .and_then(|name| {
          icon_from_name(
            name,
            item.icon_theme_path.as_deref().map(Path::new),
            &xdg_data_roots,
            &icon_theme,
            icon_size,
          )
        })
        .or_else(|| choose_pixmap(&item.icon_pixmap, icon_size))
        .map(|icon| Segment::icon(Side::Left, Class::Normal, icon).click(Click::Tray(address)))
    })
    .collect()
}
