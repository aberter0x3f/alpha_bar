use serde_json::Value;

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
  pub fn apply(&mut self, event: &Value) {
    if let Some(workspaces) = event
      .pointer("/WorkspacesChanged/workspaces")
      .and_then(Value::as_array)
    {
      self.workspaces = workspaces.iter().filter_map(parse_workspace).collect();
      self
        .workspaces
        .sort_by(|a, b| (&a.output, a.idx).cmp(&(&b.output, b.idx)));
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
      .filter(|ws| ws.output == output)
      .cloned()
      .collect()
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
