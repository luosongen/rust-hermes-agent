//! ToolRegistry — 工具注册与调度实现
//!
//! 本模块实现了 Agent 层的 `ToolDispatcher` trait，提供了：
//!
//! ## 核心类型
//! - **`Tool`** — 所有工具必须实现的 trait，包含工具元数据和执行入口
//! - **`ToolRegistry`** — 线程安全的工具注册表，负责注册、查询和分发
//!
//! ## Tool trait 方法说明
//! - `name()` — 工具唯一标识名
//! - `description()` — 工具描述，供 LLM 理解工具用途
//! - `parameters()` — JSON Schema 格式的参数定义
//! - `execute()` — 异步执行逻辑，接收 JSON 参数和 `ToolContext`
//! - `precheck()` — 可选的预检查（默认空实现）
//!
//! ## ToolRegistry 公共 API
//! - `new()` / `register()` / `unregister()` — 添加工具和移除工具
//! - `get()` / `tool_names()` — 按名称查询工具或列出所有工具
//! - `get_tool_definitions()` — 生成 `Vec<ToolDefinition>` 传给 LLM
//! - `check_all_preconditions()` — 对所有工具执行预检查
//! - `dispatch()` — 实现 `ToolDispatcher`，根据 ToolCall 分发执行
//!
//! ## 线程安全
//! 使用 `parking_lot::RwLock` 保护内部 HashMap，写线程安全，读可并发

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolDefinition, ToolDispatcher, ToolError};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 工具 trait — 所有工具必须实现此接口
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError>;
    fn precheck(&self) -> Result<(), ToolError> {
        Ok(())
    }
}

/// 工具注册表，管理已注册的工具并实现 ToolDispatcher trait 供 Agent 使用
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<T: Tool + 'static>(&self, tool: T) {
        let name = tool.name().to_string();
        self.tools.write().insert(name, Arc::new(tool));
    }

    pub fn unregister(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.write().remove(name)
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().get(name).cloned()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.read().keys().cloned().collect()
    }

    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .read()
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    pub fn check_all_preconditions(&self) -> Vec<(String, ToolError)> {
        self.tools
            .read()
            .values()
            .filter_map(|t| t.precheck().err().map(|e| (t.name().to_string(), e)))
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolDispatcher for ToolRegistry {
    fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.get_tool_definitions()
    }

    async fn dispatch(
        &self,
        call: &hermes_core::ToolCall,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let tool = self
            .get(&call.name)
            .ok_or_else(|| ToolError::NotFound(format!("Tool not found: {}", call.name)))?;
        let args = serde_json::to_value(&call.arguments).unwrap_or(serde_json::Value::Object(Default::default()));
        tool.execute(args, context).await
    }
}
