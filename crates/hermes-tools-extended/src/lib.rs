//! hermes-tools-extended — 扩展工具集
//!
//! 本 crate 提供了 AI Agent 的扩展工具实现，包括：
//!
//! ## 模块
//! - **`web_search`** — 网页搜索工具
//! - **`web_fetch`** — 网页内容抓取
//! - **`cron_scheduler`** — 定时任务调度
//! - **`mcp_server`** — MCP Server Bridge

pub mod web_search;
pub mod web_fetch;
pub mod cron_scheduler;
pub mod mcp_server;

pub use web_search::WebSearchTool;
pub use web_fetch::WebFetchTool;
pub use cron_scheduler::CronScheduler;
pub use mcp_server::McpServerBridge;

use hermes_tool_registry::ToolRegistry;

pub fn register_extended_tools(registry: &ToolRegistry) {
    registry.register(WebSearchTool::new());
    registry.register(WebFetchTool::new());
    registry.register(CronScheduler::new());
}
