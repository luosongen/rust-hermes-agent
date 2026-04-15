//! skills — 内置技能管理工具
//!
//! 本模块提供三个技能（Skill）相关工具，用于执行和搜索已注册的 Hermes 技能：
//!
//! ## 主要类型
//! - **`SkillExecuteTool`** — 技能执行工具（名称：`skill_execute`）
//!   - 参数：`name`（必填，技能名称）
//!   - 行为：根据名称从 `SkillRegistry` 获取技能内容并返回
//!   - 依赖 `SkillRegistry` 存储已加载的技能
//!
//! - **`SkillListTool`** — 技能列表工具（名称：`skill_list`）
//!   - 参数：无
//!   - 行为：返回所有已注册技能的名称列表，用换行分隔
//!
//! - **`SkillSearchTool`** — 技能搜索工具（名称：`skill_search`）
//!   - 参数：`query`（必填，搜索关键词）
//!   - 行为：在 `SkillRegistry` 中按名称或描述搜索，返回匹配技能的信息
//!
//! - **`load_skill_registry()`** — 初始化函数
//!   - 从默认目录加载所有技能文件，构建并返回 `Arc<RwLock<SkillRegistry>>`
//!   - 加载失败会记录警告但不影响程序继续运行
//!
//! ## 与其他模块的关系
//! - 实现 `hermes_tool_registry::Tool` trait
//! - 依赖外部 `hermes_skills` crate 提供 `SkillLoader` 和 `SkillRegistry`
//! - `SkillRegistry` 使用 `Arc<RwLock<...>>` 共享，可在多个工具间共用同一份注册表
//! - Agent 可通过这些工具查询和使用已注册的技能资产

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_skills::{SkillLoader, SkillRegistry};
use parking_lot::RwLock;
use std::sync::Arc;

/// Built-in skill execution tool.
///
/// Usage from agent: `skill_execute(name="skill-name")`
pub struct SkillExecuteTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillExecuteTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillExecuteTool {
    fn name(&self) -> &str {
        "skill_execute"
    }

    fn description(&self) -> &str {
        "Execute a registered Hermes skill by name, returning its content"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to execute"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let name = args
            .pointer("/name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'name' argument".into()))?;

        let registry = self.registry.read();
        let skill = registry
            .get(name)
            .ok_or_else(|| ToolError::NotFound(format!("skill not found: {}", name)))?;

        Ok(skill.content.clone())
    }
}

/// Built-in skill list tool.
///
/// Usage from agent: `skill_list()`
pub struct SkillListTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillListTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry: Arc::clone(&registry),
        }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillListTool {
    fn name(&self) -> &str {
        "skill_list"
    }

    fn description(&self) -> &str {
        "List all available Hermes skills"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        _args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let registry = self.registry.read();
        let names = registry.names();
        Ok(names.join("\n"))
    }
}

/// Built-in skill search tool.
///
/// Usage from agent: `skill_search(query="search term")`
pub struct SkillSearchTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillSearchTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry: Arc::clone(&registry),
        }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillSearchTool {
    fn name(&self) -> &str {
        "skill_search"
    }

    fn description(&self) -> &str {
        "Search available Hermes skills by name or description"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let query = args
            .pointer("/query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'query' argument".into()))?;

        let registry = self.registry.read();
        let results = registry.search(query);

        let output = results
            .iter()
            .map(|s| format!("# {}\n{}\n", s.metadata.name, s.metadata.description))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(output)
    }
}

/// Initialize skill registry by loading skills from default directories.
pub fn load_skill_registry() -> Arc<RwLock<SkillRegistry>> {
    let loader = SkillLoader::new(SkillLoader::default_dirs());
    let skills = loader.load_all().unwrap_or_default();
    let registry = Arc::new(RwLock::new(SkillRegistry::new()));
    let reg: Arc<RwLock<SkillRegistry>> = Arc::clone(&registry);
    for skill in skills {
        if let Err(e) = reg.write().register(skill) {
            tracing::warn!("Failed to register skill: {}", e);
        }
    }
    registry
}
