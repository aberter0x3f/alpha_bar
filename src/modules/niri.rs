use serde_json::Value;
use std::{
  io::{self, BufRead, BufReader, Write},
  os::unix::net::UnixStream,
  path::Path,
  sync::mpsc::Sender,
  thread,
};

use crate::{
  runtime::Update,
  state::{Class, Segment, Side},
};

pub fn spawn(tx: Sender<Update>) {
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

fn open_event_stream(path: impl AsRef<Path>) -> io::Result<UnixStream> {
  let mut stream = UnixStream::connect(path)?;
  stream.write_all(b"\"EventStream\"\n")?;
  stream.flush()?;

  let mut line = String::new();
  BufReader::new(stream.try_clone()?).read_line(&mut line)?;
  if line.trim_end() == r#"{"Ok":"Handled"}"# {
    Ok(stream)
  } else {
    Err(io::Error::other("niri rejected event stream"))
  }
}

fn request_workspaces(path: impl AsRef<Path>) -> io::Result<Vec<Workspace>> {
  let mut stream = UnixStream::connect(path)?;
  stream.write_all(b"\"Workspaces\"\n")?;
  stream.flush()?;
  let mut line = String::new();
  BufReader::new(stream).read_line(&mut line)?;
  let value = serde_json::from_str(&line).map_err(io::Error::other)?;
  parse_workspaces_response(&value).ok_or_else(|| io::Error::other("niri rejected Workspaces"))
}

fn parse_workspaces_response(value: &Value) -> Option<Vec<Workspace>> {
  value
    .pointer("/Ok/Workspaces")
    .and_then(Value::as_array)
    .map(|workspaces| parse_workspaces(workspaces))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Workspace {
  id: u64,
  idx: u64,
  name: String,
  output: String,
  active: bool,
  focused: bool,
  urgent: bool,
  empty: bool,
}

#[derive(Default)]
struct NiriState {
  workspaces: Vec<Workspace>,
}

impl NiriState {
  pub fn set_workspaces(&mut self, workspaces: Vec<Workspace>) {
    self.workspaces = workspaces;
    self.sort_workspaces();
  }

  pub fn apply(&mut self, event: &Value) {
    if let Some(workspaces) = event
      .pointer("/WorkspacesChanged/workspaces")
      .and_then(Value::as_array)
    {
      self.workspaces = parse_workspaces(workspaces);
      self.sort_workspaces();
    } else if let Some(payload) = event.get("WorkspaceActivated") {
      self.activate(payload);
    } else if let Some(payload) = event.get("WorkspaceActiveWindowChanged") {
      self.set_empty(payload);
    } else if let Some(payload) = event.get("WorkspaceUrgencyChanged") {
      self.set_urgent(payload);
    }
  }

  pub fn visible_workspaces(&self, output: &str) -> Vec<Workspace> {
    self
      .workspaces
      .iter()
      .filter(|ws| ws.output == output && (!ws.empty || ws.active || ws.focused || ws.urgent))
      .cloned()
      .collect()
  }

  pub fn workspace_segments(&self, output: &str) -> Vec<Segment> {
    self
      .visible_workspaces(output)
      .into_iter()
      .map(|ws| Segment::new(Side::Left, workspace_class(&ws), ws.name))
      .collect()
  }

  pub fn focused_output(&self) -> Option<&str> {
    self
      .workspaces
      .iter()
      .find(|ws| ws.focused)
      .map(|ws| ws.output.as_str())
  }

  fn activate(&mut self, payload: &Value) {
    let Some(id) = payload.get("id").and_then(Value::as_u64) else {
      return;
    };
    let focused = payload
      .get("focused")
      .and_then(Value::as_bool)
      .unwrap_or(false);
    let Some(output) = self
      .workspaces
      .iter()
      .find(|ws| ws.id == id)
      .map(|ws| ws.output.clone())
    else {
      return;
    };
    for ws in &mut self.workspaces {
      let is_target = ws.id == id;
      if ws.output == output {
        ws.active = is_target;
      }
      if focused {
        ws.focused = is_target;
      }
    }
  }

  fn set_empty(&mut self, payload: &Value) {
    let Some(id) = payload.get("workspace_id").and_then(Value::as_u64) else {
      return;
    };
    if let Some(ws) = self.workspaces.iter_mut().find(|ws| ws.id == id) {
      ws.empty = payload.get("active_window_id").is_some_and(Value::is_null);
    }
  }

  fn set_urgent(&mut self, payload: &Value) {
    let Some(id) = payload.get("id").and_then(Value::as_u64) else {
      return;
    };
    let urgent = payload
      .get("urgent")
      .and_then(Value::as_bool)
      .unwrap_or(false);
    if let Some(ws) = self.workspaces.iter_mut().find(|ws| ws.id == id) {
      ws.urgent = urgent;
    }
  }

  fn sort_workspaces(&mut self) {
    self
      .workspaces
      .sort_by(|a, b| (&a.output, a.idx).cmp(&(&b.output, b.idx)));
  }
}

fn workspace_class(ws: &Workspace) -> Class {
  if ws.urgent {
    Class::Critical
  } else if ws.active || ws.focused {
    Class::Active
  } else {
    Class::Normal
  }
}

fn parse_workspace(value: &Value) -> Option<Workspace> {
  let id = value.get("id")?.as_u64()?;
  let idx = value.get("idx")?.as_u64()?;
  Some(Workspace {
    id,
    idx,
    name: value
      .get("name")
      .and_then(Value::as_str)
      .map_or_else(|| idx.to_string(), str::to_owned),
    output: value.get("output")?.as_str()?.to_owned(),
    active: value
      .get("is_active")
      .and_then(Value::as_bool)
      .unwrap_or(false),
    focused: value
      .get("is_focused")
      .and_then(Value::as_bool)
      .unwrap_or(false),
    urgent: value
      .get("is_urgent")
      .and_then(Value::as_bool)
      .unwrap_or(false),
    empty: value.get("active_window_id").is_some_and(Value::is_null),
  })
}

fn parse_workspaces(workspaces: &[Value]) -> Vec<Workspace> {
  workspaces.iter().filter_map(parse_workspace).collect()
}

#[cfg(test)]
mod tests {
  use super::*;
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

    let stream = open_event_stream(&path).unwrap();
    drop(stream);
    server.join().unwrap();
    let _ = std::fs::remove_file(path);
  }
}
