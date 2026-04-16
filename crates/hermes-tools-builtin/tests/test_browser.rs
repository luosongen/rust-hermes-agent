use hermes_tools_builtin::browser_tools::{BrowserSessionStore, BrowserToolCore};
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
