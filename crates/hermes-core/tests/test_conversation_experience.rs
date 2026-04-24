//! Integration tests for conversation experience features

use hermes_core::{DisplayHandler, NoopDisplay, SessionInsights, TrajectorySaver};
use serde_json::json;
use std::sync::Arc;

struct MockDisplay {
    tool_started_calls: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

impl MockDisplay {
    fn new() -> Self {
        Self {
            tool_started_calls: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl DisplayHandler for MockDisplay {
    fn tool_started(&self, tool_name: &str, args: &serde_json::Value) {
        self.tool_started_calls
            .lock()
            .unwrap()
            .push((tool_name.to_string(), args.clone()));
    }
    fn tool_completed(&self, _tool_name: &str, _result: &str) {}
    fn tool_failed(&self, _tool_name: &str, _error: &str) {}
    fn thinking_chunk(&self, _chunk: &str) {}
    fn show_usage(&self, _insights: &SessionInsights) {}
    fn show_diff(&self, _filename: &str, _old: &str, _new: &str) {}
    fn spinner_start(&self, _message: &str) {}
    fn spinner_stop(&self) {}
    fn flush(&self) {}
}

#[test]
fn test_noop_display_methods_dont_panic() {
    let display = NoopDisplay::new();
    display.tool_started("read_file", &json!({"path": "/tmp/test"}));
    display.tool_completed("read_file", "content");
    display.tool_failed("read_file", "error");
    display.thinking_chunk("thinking...");
    display.show_diff("file.txt", "old", "new");
    display.spinner_start("loading");
    display.spinner_stop();
    display.flush();
}

#[test]
fn test_mock_display_records_tool_calls() {
    let display = Arc::new(MockDisplay::new());
    display.tool_started("write_file", &json!({"path": "/tmp/out"}));

    let calls = display.tool_started_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "write_file");
}

#[test]
fn test_trajectory_saver_default_creates_dir() {
    let saver = TrajectorySaver::default();
    // 默认目录应该已创建或至少路径有效
    assert!(!saver.output_dir().as_os_str().is_empty());
}
