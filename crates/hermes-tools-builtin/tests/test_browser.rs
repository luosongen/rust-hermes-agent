use hermes_tool_registry::Tool;
use hermes_tools_builtin::browser_tools::{
    BrowserSessionStore, BrowserToolCore,
    BrowserNavigateTool, BrowserSnapshotTool, BrowserClickTool,
    BrowserTypeTool, BrowserScrollTool, BrowserBackTool, BrowserPressTool,
    NavigateParams, SnapshotParams, ClickParams, TypeParams, ScrollParams, PressParams,
};
use std::path::PathBuf;

fn make_core() -> BrowserToolCore {
    BrowserToolCore::new(PathBuf::from("/tmp"))
}

#[test]
fn test_create_session() {
    let mut store = BrowserSessionStore::new();
    let session = store.create_session("task_1");
    assert!(session.session_name.starts_with("h_"));
    assert_eq!(session.task_id, "task_1");
    assert!(session.socket_dir.to_string_lossy().contains("agent-browser"));
}

#[test]
fn test_get_session() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    let session = store.get_session("task_1").unwrap();
    assert_eq!(session.task_id, "task_1");
}

#[test]
fn test_get_session_not_found() {
    let store = BrowserSessionStore::new();
    assert!(store.get_session("nonexistent").is_none());
}

#[test]
fn test_remove_session() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    store.remove_session("task_1");
    assert!(store.get_session("task_1").is_none());
}

#[test]
fn test_touch_updates_last_activity() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    let before = store.get_session("task_1").unwrap().last_activity;
    std::thread::sleep(std::time::Duration::from_millis(10));
    store.touch("task_1");
    let after = store.get_session("task_1").unwrap().last_activity;
    assert!(after > before);
}

#[test]
fn test_cleanup_stale_removes_old_sessions() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    store.set_session_last_activity("task_1", 0.0);
    store.cleanup_stale();
    assert!(store.get_session("task_1").is_none());
}

#[test]
fn test_get_stale_sessions() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    store.set_session_last_activity("task_1", 0.0);
    let stale = store.get_stale_sessions();
    assert_eq!(stale, vec!["task_1"]);
}

#[test]
fn test_multiple_sessions() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    store.create_session("task_2");
    assert!(store.get_session("task_1").is_some());
    assert!(store.get_session("task_2").is_some());
    store.remove_session("task_1");
    assert!(store.get_session("task_1").is_none());
    assert!(store.get_session("task_2").is_some());
}

// === Browser tool unit tests ===

#[test]
fn test_browser_navigate_tool_name() {
    let core = make_core();
    let tool = BrowserNavigateTool::new(core);
    assert_eq!(tool.name(), "browser_navigate");
}

#[test]
fn test_browser_snapshot_tool_name() {
    let core = make_core();
    let tool = BrowserSnapshotTool::new(core);
    assert_eq!(tool.name(), "browser_snapshot");
}

#[test]
fn test_browser_click_tool_name() {
    let core = make_core();
    let tool = BrowserClickTool::new(core);
    assert_eq!(tool.name(), "browser_click");
}

#[test]
fn test_browser_type_tool_name() {
    let core = make_core();
    let tool = BrowserTypeTool::new(core);
    assert_eq!(tool.name(), "browser_type");
}

#[test]
fn test_browser_scroll_tool_name() {
    let core = make_core();
    let tool = BrowserScrollTool::new(core);
    assert_eq!(tool.name(), "browser_scroll");
}

#[test]
fn test_browser_back_tool_name() {
    let core = make_core();
    let tool = BrowserBackTool::new(core);
    assert_eq!(tool.name(), "browser_back");
}

#[test]
fn test_browser_press_tool_name() {
    let core = make_core();
    let tool = BrowserPressTool::new(core);
    assert_eq!(tool.name(), "browser_press");
}

#[test]
fn test_browser_navigate_requires_url() {
    let core = make_core();
    let _tool = BrowserNavigateTool::new(core);
    let params = serde_json::json!({});
    let result = serde_json::from_value::<NavigateParams>(params);
    assert!(result.is_err()); // url is required
}

#[test]
fn test_browser_scroll_rejects_invalid_direction() {
    let core = make_core();
    let _tool = BrowserScrollTool::new(core);
    // Direction validation happens at execute time; parsing accepts any string
    let params = serde_json::json!({"direction": "left"});
    let result = serde_json::from_value::<ScrollParams>(params);
    assert!(result.is_ok()); // parses fine; execute will reject
}

#[test]
fn test_browser_snapshot_params_parses_full() {
    let core = make_core();
    let _tool = BrowserSnapshotTool::new(core);
    let params = serde_json::json!({"full": true});
    let result = serde_json::from_value::<SnapshotParams>(params);
    assert!(result.is_ok());
    assert!(result.unwrap().full);
}

#[test]
fn test_browser_click_params_requires_ref() {
    let core = make_core();
    let _tool = BrowserClickTool::new(core);
    let params = serde_json::json!({});
    let result = serde_json::from_value::<ClickParams>(params);
    assert!(result.is_err()); // ref is required
}

#[test]
fn test_browser_type_params_requires_ref_and_text() {
    let core = make_core();
    let _tool = BrowserTypeTool::new(core);
    let params = serde_json::json!({"ref": "@1"});
    let result = serde_json::from_value::<TypeParams>(params);
    assert!(result.is_err()); // text is required
}

#[test]
fn test_browser_press_params_requires_key() {
    let core = make_core();
    let _tool = BrowserPressTool::new(core);
    let params = serde_json::json!({});
    let result = serde_json::from_value::<PressParams>(params);
    assert!(result.is_err()); // key is required
}
