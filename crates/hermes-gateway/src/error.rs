//! ## hermes-gateway/error
//!
//! 网关层的错误类型定义。
//!
//! 所有错误均为结构化错误，包含具体的错误原因字符串，
//! 方便日志记录和向上层传播。

use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Webhook verification failed: {0}")]
    VerificationFailed(String),

    #[error("Failed to parse inbound message: {0}")]
    ParseError(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Outbound error: {0}")]
    OutboundError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}
