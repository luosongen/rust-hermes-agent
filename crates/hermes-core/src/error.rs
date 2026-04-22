//! 错误类型模块
//!
//! 本模块定义了 hermes-core 中所有自定义错误类型，基于 `thiserror` 库实现。
//!
//! ## 主要错误类型
//! - **AgentError**: Agent 主循环的错误（Provider/Tool/Storage/Session/Platform 错误的汇总）
//! - **ProviderError**: LLM Provider 调用中的错误（API 错误、认证失败、限流、上下文过长等）
//! - **ToolError**: 工具执行错误（执行失败、参数无效、缺少环境变量、权限拒绝、超时等）
//! - **SessionError**: 会话管理错误（会话不存在、过期、损坏）
//! - **PlatformError**: 消息平台适配器错误（连接失败、认证、限流、消息格式错误、Webhook 验证失败）
//!
//! ## ProviderError 的辅助方法
//! - `is_retryable()` — 判断错误是否值得重试（限流/API/网络错误返回 true）
//! - `retry_after_secs()` — 获取建议的重试等待秒数（仅限流错误有值）
//!
//! ## 与其他模块的关系
//! - 所有错误类型被 `lib.rs` 重新导出（`pub use error::*`）
//! - `StorageError` 来自外部的 `hermes-error` crate
//! - 各错误类型被对应的模块（`agent.rs`、`provider.rs`、`tool_dispatcher.rs` 等）使用

use thiserror::Error;
pub use hermes_error::StorageError;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    #[error("Platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Iteration budget exhausted")]
    IterationExhausted,
    #[error("Content was filtered")]
    ContentFiltered,
    #[error("Unknown finish reason")]
    UnknownFinishReason,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Billing error: {0}")]
    BillingError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
}

impl AgentError {
    /// Returns true if this error is transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        match self {
            AgentError::Provider(err) => err.is_retryable(),
            AgentError::NetworkError(_) => true,
            AgentError::TimeoutError(_) => true,
            _ => false,
        }
    }

    /// Returns the suggested retry-after seconds, if known.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            AgentError::Provider(err) => err.retry_after_secs(),
            _ => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Authentication failed")]
    Auth,
    #[error("Rate limited, retry after {0}s")]
    RateLimit(u64),
    #[error("Context length exceeded")]
    ContextTooLarge,
    #[error("Invalid model: {0}")]
    InvalidModel(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

impl ProviderError {
    /// Returns true if this error is transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::RateLimit(_) => true,
            ProviderError::Api(_) => true,
            ProviderError::Network(_) => true,
            ProviderError::Auth
            | ProviderError::InvalidModel(_)
            | ProviderError::ContextTooLarge => false,
        }
    }

    /// Returns the suggested retry-after seconds, if known.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimit(s) => Some(*s),
            _ => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Missing required environment: {0}")]
    MissingEnv(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Timeout: {0}")]
    Timeout(String),
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Session expired")]
    Expired,
    #[error("Session corrupted: {0}")]
    Corrupted(String),
}

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Authentication failed")]
    Auth,
    #[error("Rate limited")]
    RateLimit,
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    #[error("Webhook verification failed")]
    WebhookVerificationFailed,
    #[error("Message not found: {0}")]
    MessageNotFound(String),
}
