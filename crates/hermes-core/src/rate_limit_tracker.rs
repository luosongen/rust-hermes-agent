//! Rate Limit Tracker — 从响应头捕获速率限制状态
//!
//! 记录限流事件并输出 JSON 格式日志。

use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rate Limit 事件
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitEvent {
    pub event: String,
    pub provider: String,
    pub retry_after_secs: u64,
    pub timestamp: f64,
}

/// Tracker 实现
pub struct RateLimitTracker;

impl RateLimitTracker {
    pub fn new() -> Self {
        Self
    }

    /// 记录限流事件并输出 JSON 日志到 stdout
    pub fn record(&self, provider: &str, retry_after: u64) {
        let event = RateLimitEvent {
            event: "rate_limited".to_string(),
            provider: provider.to_string(),
            retry_after_secs: retry_after,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        };

        println!("{}", serde_json::to_string(&event).unwrap());
    }
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}
