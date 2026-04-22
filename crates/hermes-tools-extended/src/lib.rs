//! hermes-tools-extended — 扩展工具集
//!
//! 本 crate 提供了 AI Agent 的扩展工具实现，包括：
//!
//! ## 模块
//! - **`web_search`** — 网页搜索工具
//! - **`web_fetch`** — 网页内容抓取
//! - **`cron_scheduler`** — 定时任务调度
//! - **`mcp_server`** — MCP Server Bridge
//! - **`mcp_client`** — MCP Client Bridge

pub mod web_search;
pub mod web_fetch;
pub mod cron_scheduler;
pub mod mcp_server;
pub mod mcp_client;
pub mod cli_executor;
pub mod vision;
pub mod memory;
pub mod delegate_tool;
pub mod code_execution;
pub mod image_generation;
pub mod transcription;
pub mod homeassistant;
pub mod mixture_of_agents;
pub mod skills;
pub mod rl_training;

pub use web_search::{WebSearchTool, SearchResult};
pub use web_fetch::WebFetchTool;
pub use cron_scheduler::CronScheduler;
pub use mcp_server::McpServerBridge;
pub use mcp_client::{McpClientBridge, McpClientDispatcher, McpTool};
pub use cli_executor::{CliExecutor, ExecutorConfig, ExecutionResult};
pub use vision::VisionTool;
pub use memory::MemoryTool;
pub use delegate_tool::DelegateTool;
pub use code_execution::CodeExecutionTool;
pub use image_generation::{ImageGenerationTool, ImageSize};
pub use homeassistant::HomeAssistantTool;
pub use mixture_of_agents::MixtureOfAgentsTool;
pub use skills::SkillsTool;
pub use rl_training::RLTrainingTool;

use hermes_core::LlmProvider;
use hermes_memory::SqliteSessionStore;
use hermes_tool_registry::ToolRegistry;
use std::sync::Arc;

pub fn register_extended_tools(
    registry: Arc<ToolRegistry>,
    llm_provider: Arc<dyn LlmProvider>,
    session_store: Arc<SqliteSessionStore>,
) {
    registry.register(WebSearchTool::new());
    registry.register(WebFetchTool::new());
    
    // 创建并配置 CronScheduler
    let mut cron_scheduler = CronScheduler::new();
    cron_scheduler.set_tool_registry(Arc::clone(&registry));
    cron_scheduler.start();
    registry.register(cron_scheduler);
    
    registry.register(CliExecutor::new(ExecutorConfig::default()));
    registry.register(VisionTool::new(llm_provider));
    registry.register(MemoryTool::new(session_store));
    registry.register(HomeAssistantTool::new());
    registry.register(MixtureOfAgentsTool::new());
    registry.register(SkillsTool::new());
}
