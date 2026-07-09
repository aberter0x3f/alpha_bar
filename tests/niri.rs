use alpha_bar::niri::NiriState;
use serde_json::json;

#[test]
fn workspace_events() {
  let mut state = NiriState::default();

  state.apply(&json!({
    "WorkspacesChanged": { "workspaces": [
      {"id": 1, "idx": 1, "name": "1", "output": "eDP-1", "is_active": true, "is_focused": true, "active_window_id": 7},
      {"id": 2, "idx": 2, "name": "2", "output": "eDP-1", "is_active": false, "is_focused": false, "active_window_id": null}
    ]}
  }));
  state.apply(&json!({"WorkspaceActivated": {"id": 2, "focused": true}}));

  let visible = state.visible_workspaces("eDP-1");
  assert_eq!(visible.len(), 2);
  assert!(!visible[0].active);
  assert!(visible[1].active);
  assert!(visible[1].focused);
  assert!(visible[1].empty);
}

#[test]
fn unknown_events() {
  let mut state = NiriState::default();
  state.apply(&json!({"FutureEvent": {"field": 1}}));
  assert!(state.visible_workspaces("eDP-1").is_empty());
}
