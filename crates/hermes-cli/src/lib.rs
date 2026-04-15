//! Hermes CLI 工具库
//!
//! 提供命令行界面的公共接口，供其他 crate 或测试引用。
//!
//! ## 模块
//! - `chat`: 交互式聊天 REPL 的核心逻辑（`run_chat` 函数）
//! - `commands`: 使用 `clap` 定义的所有 CLI 命令结构（`Cli`、`Commands` 等）
//!
//! ## 公共导出
//! - `Cli`: 主 CLI 解析结构
//! - `Commands`: 所有子命令的枚举
//! - `run_chat`: 启动交互式聊天的异步函数
//!
//! ## 依赖关系
//! - 依赖 `hermes-core` 中的 `Agent`、`LlmProvider` 等核心类型
//! - 依赖 `hermes-memory` 中的 `SessionStore` trait 和 `SqliteSessionStore`
//! - 依赖 `hermes-provider` 中的 `OpenAiProvider`
//! - 依赖 `hermes-tool-registry` 和 `hermes-tools-builtin` 提供工具能力

pub mod chat;
pub mod commands;

pub use commands::*;
