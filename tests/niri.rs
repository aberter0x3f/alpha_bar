use alpha_bar::{
  niri::{NiriState, parse_workspaces_response},
  state::Class,
};
use serde_json::json;
use std::{
  io::{BufRead, BufReader, Write},
  os::unix::net::UnixListener,
  thread,
};

#[test]
fn workspace_events() {
  let mut state = NiriState::default();

  state.apply(&json!({
    "WorkspacesChanged": { "workspaces": [
      {"id": 1, "idx": 1, "name": "1", "output": "eDP-1", "is_active": true, "is_focused": true, "active_window_id": null},
      {"id": 2, "idx": 2, "name": "2", "output": "eDP-1", "is_active": false, "is_focused": false, "active_window_id": null}
    ]}
  }));
  state.apply(&json!({"WorkspaceActivated": {"id": 2, "focused": true}}));

  let visible = state.visible_workspaces("eDP-1");
  assert_eq!(visible.len(), 1);
  assert!(visible[0].active);
  assert!(visible[0].focused);
  assert!(visible[0].empty);
  assert_eq!(state.focused_output(), Some("eDP-1"));
}

#[test]
fn workspace_segments() {
  let mut state = NiriState::default();
  state.apply(&json!({
    "WorkspacesChanged": { "workspaces": [
      {"id": 1, "idx": 1, "name": "1", "output": "eDP-1", "is_active": false, "is_focused": false, "active_window_id": 7},
      {"id": 2, "idx": 2, "name": "2", "output": "eDP-1", "is_active": true, "is_focused": true, "active_window_id": null},
      {"id": 3, "idx": 3, "name": "3", "output": "eDP-1", "is_active": false, "is_focused": false, "is_urgent": true, "active_window_id": null},
      {"id": 4, "idx": 4, "name": "4", "output": "eDP-1", "is_active": false, "is_focused": false, "active_window_id": null}
    ]}
  }));

  let segments = state.workspace_segments("eDP-1");
  assert_eq!(
    segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
    ["1", "2", "3"]
  );
  assert_eq!(segments[0].class, Class::Normal);
  assert_eq!(segments[1].class, Class::Active);
  assert_eq!(segments[2].class, Class::Critical);
}

#[test]
fn initial_workspaces_response() {
  let mut state = NiriState::default();
  let response = json!({"Ok": {"Workspaces": [
    {"id": 3, "idx": 3, "name": "3", "output": "HDMI-A-5", "is_active": false, "is_focused": false, "active_window_id": null},
    {"id": 1, "idx": 1, "name": "1", "output": "HDMI-A-5", "is_active": false, "is_focused": false, "active_window_id": 84},
    {"id": 2, "idx": 2, "name": "2", "output": "HDMI-A-5", "is_active": true, "is_focused": true, "active_window_id": 153},
    {"id": 4, "idx": 4, "name": "4", "output": "HDMI-A-5", "is_active": false, "is_focused": false, "is_urgent": true, "active_window_id": null}
  ]}});

  state.set_workspaces(parse_workspaces_response(&response).unwrap());

  let segments = state.workspace_segments("HDMI-A-5");
  assert_eq!(
    segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(),
    ["1", "2", "4"]
  );
}

#[test]
fn unknown_events() {
  let mut state = NiriState::default();
  state.apply(&json!({"FutureEvent": {"field": 1}}));
  assert!(state.visible_workspaces("eDP-1").is_empty());
}

#[test]
fn event_stream_socket() {
  let path = std::env::temp_dir().join(format!("alpha-bar-niri-{}.sock", std::process::id()));
  let _ = std::fs::remove_file(&path);
  let listener = UnixListener::bind(&path).unwrap();
  let server = thread::spawn(move || {
    let (mut stream, _) = listener.accept().unwrap();
    let mut line = String::new();
    BufReader::new(stream.try_clone().unwrap())
      .read_line(&mut line)
      .unwrap();
    assert_eq!(line, "\"EventStream\"\n");
    stream.write_all(b"{\"Ok\":\"Handled\"}\n").unwrap();
  });

  let stream = alpha_bar::niri::open_event_stream(&path).unwrap();
  drop(stream);
  server.join().unwrap();
  let _ = std::fs::remove_file(&path);
}
