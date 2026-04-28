//! 工具调度器抽象接口模块
//!
//! 本模块定义了 `ToolDispatcher` trait，作为工具注册的抽象层。
//!
//! ## 设计动机
//! Agent 自身在 `hermes-core` 中，而具体工具实现在 `hermes-tool-registry` 中。
//! `hermes-tool-registry` 依赖 `hermes-core`（因为需要 `ToolCall` 等类型），
//! 因此不能反过来让 `hermes-core` 直接依赖 `hermes-tool-registry`。
//! `ToolDispatcher` trait 作为接口打破了这一循环依赖。
//!
//! ## 主要类型
//! - **ToolDispatcher**（trait）: 异步工具调度接口
//!   - `get_definitions()` — 返回要发送给 LLM 的工具定义列表
//!   - `dispatch()` — 执行具体的工具调用并返回字符串结果
//!
//! ## 与其他模块的关系
//! - 由 `hermes-tool-registry` 中的 `ToolRegistry` 实现
//! - 被 `agent.rs` 使用来获取工具定义和执行工具调用
//! - `ToolCall`、`ToolDefinition`、`ToolContext`、`ToolError` 来自 `types.rs` 和 `error.rs`

use crate::{ToolCall, ToolContext, ToolDefinition, ToolError};
use async_trait::async_trait;

/// 工具调度器 trait
///
/// 工具注册的抽象接口，打破 hermes-core 与 hermes-tool-registry 之间的循环依赖。
#[async_trait]
pub trait ToolDispatcher: Send + Sync {
    /// 返回要发送给 LLM 的工具定义列表
    fn get_definitions(&self) -> Vec<ToolDefinition>;

    /// 执行工具调用并返回字符串结果
    async fn dispatch(&self, call: &ToolCall, context: ToolContext) -> Result<String, ToolError>;
}
