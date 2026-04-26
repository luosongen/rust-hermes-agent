//! hermes-memory — 会话与消息持久化模块
//!
//! 本模块负责存储和管理 AI 对话会话（Session）及其消息（Message）。它是 rust-hermes-agent
//! 的记忆子系统，将所有对话上下文持久化到 SQLite 数据库中。
//!
//! ## 核心类型
//!
//! - **`Session`** — 表示一个完整的对话会话，包含会话元数据、token 统计、计费信息和状态。
//! - **`NewSession`** — 创建新会话时使用的轻量输入类型。
//! - **`Message`** — 会话中的一条消息，包含角色、内容、工具调用信息和 token 计数。
//! - **`NewMessage`** — 追加消息时使用的轻量输入类型。
//! - **`SearchResult`** — 全文搜索的结果，包含消息片段。
//! - **`SessionStore`** — 会话存储的 trait，定义了所有持久化操作的接口。
//!
//! ## 与其他模块的关系
//!
//! - **`hermes-core`** — 通过 `SessionStore` trait 使用本模块。Agent 在处理每次对话时会
//!   调用 `append_message` 追加消息，并在需要时通过 `get_messages` 获取历史上下文。
//! - **`hermes-provider`** — Session 中记录的 token 数量和计费信息由 LLM Provider 提供。
//! - **`hermes-gateway`** — 平台适配器（如 Telegram、WeCom）通过会话 ID 将消息
//!   路由到对应的 Session。
//!
//! ## 数据库后端
//!
//! 当前唯一实现是 `SqliteSessionStore`（见 `sqlite_store.rs`），它使用 SQLite
//! 存储会话和消息，并通过 FTS5 虚拟表支持全文搜索。

pub mod builtin;
pub mod compressed;
pub mod compression;
pub mod compression_config;
pub mod compression_error;
pub mod session;
pub mod sqlite_store;
pub mod memory_manager;
pub mod search;
pub mod summarizer;
#[cfg(test)]
mod tests;

pub use sqlite_store::SqliteSessionStore;
pub use session::*;
pub use memory_manager::{MemoryManager, MemoryProvider};
pub use builtin::BuiltinMemoryProvider;
pub use search::{sanitize_fts_query, SessionSummarizer};
