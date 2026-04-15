//! hermes-tool-registry — 工具注册中心核心模块
//!
//! 本模块是整个工具系统的中心入口，负责：
//!
//! ## 模块结构
//! - **`registry`** — 核心组件，包含 `Tool` trait 和 `ToolRegistry` 类型
//!
//! ## 主要类型
//! - **`Tool`** trait — 所有工具必须实现的接口，定义名称、描述、参数模式和执行逻辑
//! - **`ToolRegistry`** — 工具的注册、管理与调度中心
//! - **`ToolDispatcher`** trait — Agent 调用工具的统一抽象接口
//!
//! ## 与其他模块的关系
//! - 被 `hermes-core` 定义为 `ToolDispatcher` trait 的实现者
//! - 被 `hermes-tools-builtin` 中的具体工具实现（通过 `Tool` trait）
//! - 被 Agent 层调用，用于获取工具定义列表和分发工具调用请求
//!
//! ## 使用方式
//! ```ignore
//! use hermes_tool_registry::{ToolRegistry, Tool};
//!
//! let registry = ToolRegistry::new();
//! registry.register(MyTool::new());
//! let definitions = registry.get_tool_definitions(); // 供 LLM 使用
//! ```

pub mod registry;

pub use registry::*;
