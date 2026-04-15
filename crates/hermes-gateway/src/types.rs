//! ## hermes-gateway/types
//!
//! 网关层的数据类型定义。
//!
//! 本模块重导出 `hermes_core` 中定义的共享类型，并额外定义了
//! `AgentResponse` 作为网关内部使用的 Agent 响应包装结构。

// Re-export gateway types from hermes-core
pub use hermes_core::gateway::{GatewayError, InboundMessage};

/// Agent response wrapper for internal use.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub session_id: String,
}
