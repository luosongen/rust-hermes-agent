//! ## hermes-memory/session
//!
//! 会话和消息的领域类型定义。
//!
//! 本模块定义了会话存储的核心数据结构：
//! - [`Session`] — 会话实体，包含元数据、token 统计和计费信息
//! - [`NewSession`] — 创建新会话时使用的输入类型
//! - [`Message`] — 会话中的消息记录
//! - [`NewMessage`] — 追加消息时的输入类型
//! - [`SearchResult`] — 消息搜索结果
//! - [`SessionStore`] — 会话存储抽象 trait

use serde::{Deserialize, Serialize};

/// 会话实体 — 存储会话的元数据和统计信息。
///
/// 包含会话 ID、来源、用户 ID、使用的模型、系统提示词、时间戳、
/// token 使用量统计（input/output/cache/reasoning）、计费信息等字段。
/// `message_count` 和 `tool_call_count` 在消息追加时自动递增。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub source: String,
    pub user_id: Option<String>,
    pub model: Option<String>,
    pub model_config: Option<String>,
    pub system_prompt: Option<String>,
    pub parent_session_id: Option<String>,
    pub started_at: f64,
    pub ended_at: Option<f64>,
    pub end_reason: Option<String>,
    pub message_count: usize,
    pub tool_call_count: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_read_tokens: usize,
    pub cache_write_tokens: usize,
    pub reasoning_tokens: usize,
    pub billing_provider: Option<String>,
    pub billing_base_url: Option<String>,
    pub billing_mode: Option<String>,
    pub estimated_cost_usd: Option<f64>,
    pub actual_cost_usd: Option<f64>,
    pub title: Option<String>,
    pub metadata: Option<String>,
}

/// 创建新会话时的输入类型。
#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: String,
    pub source: String,
    pub user_id: Option<String>,
    pub model: Option<String>,
}

/// 会话中的单条消息记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<String>,
    pub tool_name: Option<String>,
    pub timestamp: f64,
    pub token_count: Option<usize>,
    pub finish_reason: Option<String>,
    pub reasoning: Option<String>,
}

/// 追加消息时的输入类型。
#[derive(Debug, Clone)]
pub struct NewMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_calls: Option<String>,
    pub tool_name: Option<String>,
    pub timestamp: f64,
    pub token_count: Option<usize>,
    pub finish_reason: Option<String>,
    pub reasoning: Option<String>,
}

/// 消息全文搜索结果，包含匹配片段（snippet）用于预览。
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub snippet: String,
}

/// 会话存储 trait — 抽象了会话和消息的持久化操作。
///
/// 由 [`crate::SqliteSessionStore`] 具体实现。
/// 所有方法均为 async，由 sqlx 驱动 SQLite 数据库。
use async_trait::async_trait;
use hermes_error::StorageError;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create_session(&self, session: NewSession) -> Result<Session, StorageError>;
    async fn get_session(&self, session_id: &str) -> Result<Option<Session>, StorageError>;
    async fn append_message(&self, session_id: &str, message: NewMessage) -> Result<Message, StorageError>;
    async fn get_messages(&self, session_id: &str, limit: usize, offset: usize) -> Result<Vec<Message>, StorageError>;
    async fn search_messages(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, StorageError>;
}
