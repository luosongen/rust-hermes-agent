use serde::{Deserialize, Serialize};

/// Session data
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

/// New session to create
#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: String,
    pub source: String,
    pub user_id: Option<String>,
    pub model: Option<String>,
}

/// Message in a session
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

/// New message to append
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

/// Search result
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub snippet: String,
}

/// Session store trait
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
