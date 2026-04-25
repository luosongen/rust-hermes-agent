//! Watch 模块 — 文件/进程监控

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 监控事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchEventType {
    Created,
    Modified,
    Deleted,
    Any,
}

/// 监控动作
#[derive(Debug, Clone)]
pub enum WatchAction {
    /// 运行指定 Job ID
    RunJob(String),
    /// 发送通知
    SendNotification(String),
    /// 执行命令
    ExecuteCommand(String),
}

/// 监控模式
#[derive(Debug, Clone)]
pub struct WatchPattern {
    /// glob 模式
    pub pattern: String,
    /// 监控的事件类型
    pub events: Vec<WatchEventType>,
    /// 触发动作
    pub action: WatchAction,
}

impl WatchPattern {
    pub fn new(pattern: impl Into<String>, action: WatchAction) -> Self {
        Self {
            pattern: pattern.into(),
            events: vec![WatchEventType::Any],
            action,
        }
    }

    pub fn with_events(mut self, events: Vec<WatchEventType>) -> Self {
        self.events = events;
        self
    }
}

/// 监控事件
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// 事件类型
    pub event_type: WatchEventType,
    /// 触发该事件的文件路径
    pub path: PathBuf,
    /// 触发时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl WatchEvent {
    pub fn new(event_type: WatchEventType, path: PathBuf) -> Self {
        Self {
            event_type,
            path,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// 监控器状态
#[derive(Debug, Clone)]
pub struct WatchMonitorState {
    pub patterns: Vec<WatchPattern>,
    pub is_running: bool,
}

impl Default for WatchMonitorState {
    fn default() -> Self {
        Self {
            patterns: Vec::new(),
            is_running: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_pattern_new() {
        let pattern = WatchPattern::new("*.rs", WatchAction::RunJob("test".to_string()));
        assert_eq!(pattern.pattern, "*.rs");
        assert!(matches!(pattern.action, WatchAction::RunJob(_)));
    }

    #[test]
    fn test_watch_pattern_with_events() {
        let pattern = WatchPattern::new("*.rs", WatchAction::SendNotification("changed".to_string()))
            .with_events(vec![WatchEventType::Modified]);
        assert_eq!(pattern.events.len(), 1);
    }

    #[test]
    fn test_watch_event() {
        let event = WatchEvent::new(
            WatchEventType::Modified,
            PathBuf::from("/path/to/file.rs"),
        );
        assert!(matches!(event.event_type, WatchEventType::Modified));
        assert_eq!(event.path, PathBuf::from("/path/to/file.rs"));
    }
}
