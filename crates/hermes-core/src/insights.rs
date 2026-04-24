//! Insights — 会话洞察引擎
//!
//! 追踪 LLM token 消耗和工具调用记录。

use crate::Usage;
use serde::Serialize;
use std::sync::Mutex;

/// 单次工具调用记录
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub started_at: f64,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// 会话洞察数据
#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionInsights {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_read_tokens: usize,
    pub cache_write_tokens: usize,
    pub reasoning_tokens: usize,
    pub estimated_cost_usd: f64,
    pub tool_calls: Vec<ToolCallRecord>,
}

impl SessionInsights {
    pub fn new(session_id: &str, provider: &str, model: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            ..Default::default()
        }
    }
}

/// Insights tracker trait
pub trait InsightsTracker: Send + Sync {
    fn record_tool_call(&self, record: ToolCallRecord);
    fn record_usage(&self, usage: &Usage, cost_usd: f64);
    fn get_insights(&self) -> SessionInsights;
}

/// 内存存储的 tracker 实现
pub struct InMemoryInsightsTracker {
    insights: Mutex<SessionInsights>,
}

impl InMemoryInsightsTracker {
    pub fn new(session_id: &str, provider: &str, model: &str) -> Self {
        Self {
            insights: Mutex::new(SessionInsights::new(session_id, provider, model)),
        }
    }
}

impl InsightsTracker for InMemoryInsightsTracker {
    fn record_tool_call(&self, record: ToolCallRecord) {
        let mut insights = self.insights.lock();
        insights.tool_calls.push(record);
    }

    fn record_usage(&self, usage: &Usage, cost_usd: f64) {
        let mut insights = self.insights.lock();
        insights.input_tokens = usage.input_tokens;
        insights.output_tokens = usage.output_tokens;
        insights.cache_read_tokens = usage.cache_read_tokens.unwrap_or(0);
        insights.cache_write_tokens = usage.cache_write_tokens.unwrap_or(0);
        insights.reasoning_tokens = usage.reasoning_tokens.unwrap_or(0);
        insights.estimated_cost_usd = cost_usd;
    }

    fn get_insights(&self) -> SessionInsights {
        self.insights.lock().clone()
    }
}
