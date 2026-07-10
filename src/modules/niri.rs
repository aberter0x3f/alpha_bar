use serde_json::Value;
use std::{
  io::{self, BufRead, BufReader, Write},
  os::unix::net::UnixStream,
  path::Path,
};

use crate::state::{Class, Segment, Side};

pub fn open_event_stream(path: impl AsRef<Path>) -> io::Result<UnixStream> {
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

pub fn request_workspaces(path: impl AsRef<Path>) -> io::Result<Vec<Workspace>> {
  let mut stream = UnixStream::connect(path)?;
  stream.write_all(b"\"Workspaces\"\n")?;
  stream.flush()?;
  let mut line = String::new();
  BufReader::new(stream).read_line(&mut line)?;
  let value = serde_json::from_str(&line).map_err(io::Error::other)?;
  parse_workspaces_response(&value).ok_or_else(|| io::Error::other("niri rejected Workspaces"))
}

pub fn parse_workspaces_response(value: &Value) -> Option<Vec<Workspace>> {
  value
    .pointer("/Ok/Workspaces")
    .and_then(Value::as_array)
    .map(|workspaces| parse_workspaces(workspaces))
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Workspace {
  pub id: u64,
  pub idx: u64,
  pub name: String,
  pub output: String,
  pub active: bool,
  pub focused: bool,
  pub urgent: bool,
  pub empty: bool,
}

#[derive(Default)]
pub struct NiriState {
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
