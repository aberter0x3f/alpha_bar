fn main() {
  let (tx, rx) = std::sync::mpsc::channel();
  let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
  alpha_bar::runtime::spawn_workers(tx, cmd_rx);
  if let Err(err) = alpha_bar::ui::layer::run(alpha_bar::state::Snapshot::default(), rx, cmd_tx) {
    eprintln!("alpha_bar: {err}");
  }
}
