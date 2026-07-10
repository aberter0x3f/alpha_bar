pub mod app;
pub mod config;
pub mod format;
pub mod modules;
pub mod runtime;
pub mod state;
pub mod style;
pub mod ui;

pub use modules::{clock, net_speed, network, niri, power, proc_parse, tray};
