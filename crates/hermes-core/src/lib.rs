//! Hermes Core — AI Agent 核心库
//!
//! ## 模块概览
//! `hermes-core` 是 hermes-agent 的核心库，提供了 AI 对话 Agent 的完整实现。
//!
//! ### 核心子模块
//! | 模块 | 职责 |
//! |-------|------|
//! | `types` | 所有核心数据类型（消息、角色、工具调用、LLM 请求/响应等） |
//! | `agent` | Agent 主循环——与 LLM 交互、调度工具、管理会话 |
//! | `provider` | `LlmProvider` trait，所有 LLM 后端的统一抽象接口 |
//! | `tool_dispatcher` | `ToolDispatcher` trait，工具调度的抽象接口 |
//! | `retrying_provider` | 带重试逻辑和凭证池的 Provider 装饰器 |
//! | `credentials` | 多 API 密钥的凭证池管理（健康检查、轮询、冷却） |
//! | `retry` | 指数退避重试策略定义 |
//! | `error` | 所有错误类型（Agent、Provider、Tool、Session、Platform） |
//! | `gateway` | 网关和平台适配器的共享类型 |
//! | `config` | 配置系统（文件、环境变量、默认值多层级加载） |
//! | `conversation` | 会话请求/响应的高层包装 |
//!
//! ### 架构依赖关系
//! ```text
//! CLI (hermes-cli) → Agent (hermes-core) → LlmProvider (hermes-provider)
//!                 ↓
//!          ToolDispatcher → ToolRegistry → Tools (hermes-tools-builtin)
////!                 ↓
////!          SessionStore (hermes-memory) → SQLite
//! ```
//!
//! ### 关键 trait
//! - **LlmProvider**: LLM 后端统一接口（`provider.rs`）
//! - **ToolDispatcher**: 工具注册抽象接口（`tool_dispatcher.rs`）
//! - **PlatformAdapter**: 消息平台适配器接口（`gateway.rs`）
//!
//! ### 关键 re-export
//! - `Agent`, `AgentConfig` — 主 Agent 类型
//! - `RetryingProvider` — 带重试的 Provider 包装器
//! - `CredentialPool` — 凭证池管理器
//! - `RetryPolicy` — 重试策略
//! - `SessionStore` — 会话存储接口（来自 hermes-memory）

pub mod types;
pub mod error;
pub mod provider;
pub mod tool_dispatcher;
pub mod retry;
pub mod credentials;
pub mod retrying_provider;
pub mod agent;
pub mod conversation;
pub mod gateway;
pub mod config;
pub mod context_compressor;
pub mod traits;
pub mod delegate;
pub mod nudge;
pub mod compression;
pub mod routing;

pub use compression::{ToolResultPruner, Summarizer, PRUNED_TOOL_PLACEHOLDER};
pub use routing::{SmartRouter, ComplexityDetector, RouteResolution};

pub use credentials::CredentialPool;
pub use context_compressor::ContextCompressor;
pub use retrying_provider::RetryingProvider;
pub use retry::RetryPolicy;

pub use types::*;
pub use error::*;
pub use provider::LlmProvider;
pub use tool_dispatcher::ToolDispatcher;
pub use agent::Agent;
pub use agent::AgentConfig;
pub use conversation::*;
pub use gateway::*;
pub use hermes_memory::SessionStore;
pub use nudge::{NudgeConfig, NudgeService, NudgeState, NudgeTrigger, ReviewPrompts};

#[cfg(test)]
mod tests;
