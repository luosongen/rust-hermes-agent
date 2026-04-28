//! 网关消息类型模块
//!
//! ## 模块用途
//! 定义消息网关和平台适配器的共享类型。本模块位于 `hermes-core` 而非 `hermes-gateway`，
//! 是为了打破循环依赖：hermes-gateway ↔ hermes-platform-telegram ↔ hermes-gateway。
//!
//! ## 主要类型
//! - **GatewayError**: 网关操作的错误类型（Webhook 验证失败、解析错误、Agent 错误等）
//! - **InboundMessage**: 经过平台适配器解析后的规范化入站消息
//! - **PlatformAdapter**（trait）: 消息平台的抽象接口
//!
//! ## PlatformAdapter trait 方法
//! - `platform_id()` — 返回平台名称（"telegram" 或 "wecom"）
//! - `verify_webhook()` — **同步** 验证 Webhook 请求的真实性（仅检查查询参数）
//! - `parse_inbound()` — **异步** 将平台专有格式解析为规范的 `InboundMessage`
//! - `send_response()` — 将 Agent 响应发送回消息平台
//!
//! ## InboundMessage 字段说明
//! - `platform` — 来源平台名称
//! - `sender_id` — 发送者 ID
//! - `content` — 消息内容
//! - `session_id` — 关联的会话 ID
//! - `timestamp` — 消息时间戳（UTC）
//! - `raw` — 原始平台消息的 JSON 表示
//!
//! ## 与其他模块的关系
//! - 被 `hermes-gateway` 使用来注册平台路由和处理 Webhook
//! - 被 `hermes-platform-telegram` 和 `hermes-platform-wecom` 实现具体平台逻辑
//! - `ConversationResponse` 来自 `conversation.rs`

use crate::ConversationResponse;
use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 网关错误类型
#[derive(Error, Debug)]
pub enum GatewayError {
    /// Webhook 验证失败
    #[error("Webhook verification failed: {0}")]
    VerificationFailed(String),

    /// 消息解析失败
    #[error("Failed to parse inbound message: {0}")]
    ParseError(String),

    /// Agent 错误
    #[error("Agent error: {0}")]
    AgentError(String),

    /// 会话错误
    #[error("Session error: {0}")]
    SessionError(String),

    /// 出站消息错误
    #[error("Outbound error: {0}")]
    OutboundError(String),

    /// 配置错误
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// HTTP 错误
    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

/// 规范化的入站消息
///
/// 经平台适配器解析后的统一消息格式。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// 来源平台名称（如 "telegram"、"wecom"）
    pub platform: String,
    /// 发送者 ID
    pub sender_id: String,
    /// 消息内容
    pub content: String,
    /// 关联的会话 ID
    pub session_id: String,
    /// 消息时间戳（UTC）
    pub timestamp: DateTime<Utc>,
    /// 原始平台消息的 JSON 表示
    pub raw: serde_json::Value,
}

/// 平台适配器 trait
///
/// 各消息平台（Telegram、企业微信等）需实现此接口。
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// 返回平台名称（如 "telegram"、"wecom"）
    fn platform_id(&self) -> &str;

    /// 验证 Webhook 请求的真实性（同步方法，仅检查查询参数）
    fn verify_webhook(&self, request: &Request<Body>) -> bool;

    /// 将 Webhook 请求解析为规范的 InboundMessage
    async fn parse_inbound(&self, request: Request<Body>) -> Result<InboundMessage, GatewayError>;

    /// 将 Agent 响应发送回消息平台
    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError>;
}
