//! 会话请求与响应模块
//!
//! 本模块定义了用户与 Agent 之间的高层对话接口。
//!
//! ## 主要类型
//! - **ConversationRequest**: 用户发起的会话请求，包含内容、会话 ID（可选）和系统提示词
//! - **ConversationResponse**: Agent 返回的会话响应，包含内容、会话 ID 和 token 使用量
//!
//! ## 与其他模块的关系
//! - 由 `agent.rs` 中的 `Agent::run_conversation()` 实现具体逻辑
//! - 会话 ID 用于关联 `hermes-memory` 中的历史消息
//! - 响应中的 `Usage` 来自底层 LLM Provider 的统计信息

use crate::Usage;

/// Conversation request
#[derive(Debug, Clone)]
pub struct ConversationRequest {
    pub content: String,
    pub session_id: Option<String>,
    pub system_prompt: Option<String>,
}

/// Conversation response
#[derive(Debug, Clone)]
pub struct ConversationResponse {
    pub content: String,
    pub session_id: Option<String>,
    pub usage: Option<Usage>,
}
