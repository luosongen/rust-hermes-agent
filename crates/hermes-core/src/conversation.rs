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

/// 会话请求
///
/// 用户发起的对话请求，包含消息内容、可选的会话 ID 和系统提示词。
#[derive(Debug, Clone)]
pub struct ConversationRequest {
    /// 用户输入的内容
    pub content: String,
    /// 会话 ID（用于关联历史消息）
    pub session_id: Option<String>,
    /// 自定义系统提示词
    pub system_prompt: Option<String>,
}

/// 会话响应
///
/// Agent 返回的对话响应，包含生成的内容、会话 ID 和 token 使用量。
#[derive(Debug, Clone)]
pub struct ConversationResponse {
    /// Agent 生成的内容
    pub content: String,
    /// 会话 ID
    pub session_id: Option<String>,
    /// Token 使用量统计
    pub usage: Option<Usage>,
}
