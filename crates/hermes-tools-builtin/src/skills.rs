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
use hermes_skills::{SkillExecutor, SkillLoader, SkillRegistry};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 技能执行工具，支持逐步执行技能
///
/// 提供技能的启动、继续、完成和状态查询功能。
pub struct SkillExecuteTool {
    registry: Arc<RwLock<SkillRegistry>>,
    executor: Arc<SkillExecutor>,
    executions: Arc<RwLock<HashMap<String, hermes_skills::SkillExecution>>>,
}

impl SkillExecuteTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>, executor: Arc<SkillExecutor>) -> Self {
        Self {
            registry,
            executor,
            executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillExecuteTool {
    fn name(&self) -> &str {
        "skill_execute"
    }

    fn description(&self) -> &str {
        "Execute a registered Hermes skill by name with step-by-step execution support"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to execute"
                },
                "action": {
                    "type": "string",
                    "enum": ["start", "continue", "complete", "status"],
                    "description": "Action: start (begin execution), continue (get current step), complete (finish step), status (show progress)"
                }
            },
            "required": ["name", "action"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let name = args.pointer("/name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'name' argument".into()))?;

        let action = args.pointer("/action")
            .and_then(|v| v.as_str())
            .unwrap_or("start");

        let session_id = context.session_id;

        match action {
            "start" => {
                let execution = self.executor.start(name)
                    .map_err(|e| ToolError::Execution(e.to_string()))?;

                let step_content = self.executor.get_current_step(&execution)
                    .ok_or_else(|| ToolError::Execution("No steps found".into()))?;

                self.executions.write().insert(session_id.clone(), execution);

                Ok(format!(
                    "Started skill '{}'.\n\n{}\n\nRemaining:\n{}",
                    name,
                    step_content,
                    self.executor.get_remaining_steps(self.executions.read().get(&session_id).unwrap())
                ))
            }
            "continue" => {
                let mut executions = self.executions.write();
                let execution = executions.get_mut(&session_id)
                    .ok_or_else(|| ToolError::Execution("No active execution".into()))?;

                let step_content = self.executor.get_current_step(execution)
                    .ok_or_else(|| ToolError::Execution("No more steps".into()))?;

                Ok(format!(
                    "{}\n\nRemaining:\n{}",
                    step_content,
                    self.executor.get_remaining_steps(execution)
                ))
            }
            "complete" => {
                let mut executions = self.executions.write();
                let execution = executions.get_mut(&session_id)
                    .ok_or_else(|| ToolError::Execution("No active execution".into()))?;

                self.executor.complete_step(execution);

                if execution.is_complete() {
                    let summary = self.executor.get_progress_summary(execution);
                    executions.remove(&session_id);
                    Ok(format!("✅ {}", summary))
                } else {
                    let step_content = self.executor.get_current_step(execution)
                        .unwrap_or_else(|| "All steps completed!".to_string());
                    Ok(format!("{}\n\nNext:\n{}\n\nRemaining:\n{}",
                        self.executor.get_progress_summary(execution),
                        step_content,
                        self.executor.get_remaining_steps(execution)))
                }
            }
            "status" => {
                let executions = self.executions.read();
                if let Some(execution) = executions.get(&session_id) {
                    Ok(self.executor.get_progress_summary(execution))
                } else {
                    Ok("No active skill execution".to_string())
                }
            }
            _ => Err(ToolError::InvalidArgs(format!("Unknown action: {}", action)))
        }
    }
}

/// 技能列表工具
///
/// 列出所有已注册的技能名称。
/// Agent 调用方式：`skill_list()`
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

/// 技能搜索工具
///
/// 按名称或描述搜索已注册的技能。
/// Agent 调用方式：`skill_search(query="搜索关键词")`
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

/// 初始化技能注册表，从默认目录加载所有技能
///
/// 返回可在多个工具间共享的注册表引用。
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

use hermes_skills::manager::SkillManager;

/// 技能管理工具
///
/// 提供技能的创建、编辑、补丁、删除、写入文件和删除文件功能。
/// Agent 调用方式：`skill_manage(action="create", name="my-skill", content="...")`
pub struct SkillManageTool {
    manager: Arc<RwLock<SkillManager>>,
}

impl SkillManageTool {
    pub fn new(manager: Arc<RwLock<SkillManager>>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillManageTool {
    fn name(&self) -> &str {
        "skill_manage"
    }

    fn description(&self) -> &str {
        "Manage skills (create, edit, patch, delete, write_file, remove_file).
        Skills are your procedural memory - reusable approaches for recurring task types.
        Create when: complex task succeeded, errors overcome, or user asks to remember a procedure."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "edit", "patch", "delete", "write_file", "remove_file"],
                    "description": "The action to perform."
                },
                "name": {
                    "type": "string",
                    "description": "Skill name (lowercase, hyphens/underscores, max 64 chars)."
                },
                "content": {
                    "type": "string",
                    "description": "Full SKILL.md content (YAML frontmatter + markdown body). Required for 'create' and 'edit'."
                },
                "category": {
                    "type": "string",
                    "description": "Optional category/domain for organizing the skill (e.g., 'devops', 'data-science')."
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to a supporting file within the skill directory. For 'write_file'/'remove_file': required. For 'patch': optional, defaults to SKILL.md."
                },
                "file_content": {
                    "type": "string",
                    "description": "Content for the file. Required for 'write_file'."
                },
                "old_string": {
                    "type": "string",
                    "description": "Text to find in the file (required for 'patch')."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text (required for 'patch'). Can be empty string to delete."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "For 'patch': replace all occurrences instead of requiring a unique match (default: false)."
                }
            },
            "required": ["action", "name"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let action = args.pointer("/action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'action' argument".into()))?;

        let name = args.pointer("/name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'name' argument".into()))?;

        let manager = self.manager.read();

        let result: serde_json::Value = match action {
            "create" => {
                let content = args.pointer("/content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'content' argument for 'create'".into()))?;
                let category = args.pointer("/category").and_then(|v| v.as_str());
                serde_json::to_value(manager.create(name, content, category).map_err(|e| ToolError::Execution(e.to_string()))?).map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "edit" => {
                let content = args.pointer("/content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'content' argument for 'edit'".into()))?;
                serde_json::to_value(manager.edit(name, content).map_err(|e| ToolError::Execution(e.to_string()))?).map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "patch" => {
                let old_string = args.pointer("/old_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'old_string' argument for 'patch'".into()))?;
                let new_string = args.pointer("/new_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'new_string' argument for 'patch'".into()))?;
                let replace_all = args.pointer("/replace_all")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let file_path = args.pointer("/file_path").and_then(|v| v.as_str());
                serde_json::to_value(manager.patch(name, old_string, new_string, replace_all, file_path).map_err(|e| ToolError::Execution(e.to_string()))?).map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "delete" => {
                serde_json::to_value(manager.delete(name).map_err(|e| ToolError::Execution(e.to_string()))?).map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "write_file" => {
                let file_path = args.pointer("/file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'file_path' argument for 'write_file'".into()))?;
                let file_content = args.pointer("/file_content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'file_content' argument for 'write_file'".into()))?;
                serde_json::to_value(manager.write_file(name, file_path, file_content).map_err(|e| ToolError::Execution(e.to_string()))?).map_err(|e| ToolError::Execution(e.to_string()))?
            }
            "remove_file" => {
                let file_path = args.pointer("/file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'file_path' argument for 'remove_file'".into()))?;
                serde_json::to_value(manager.remove_file(name, file_path).map_err(|e| ToolError::Execution(e.to_string()))?).map_err(|e| ToolError::Execution(e.to_string()))?
            }
            _ => {
                return Err(ToolError::InvalidArgs(format!("Unknown action: {}", action)));
            }
        };

        Ok(serde_json::to_string(&result).map_err(|e| ToolError::Execution(e.to_string()))?)
    }
}

/// 初始化技能注册表和管理器，从默认目录加载技能
///
/// 返回三元组：(注册表, 管理器, 执行器)
pub fn load_skill_registry_and_manager() -> (Arc<RwLock<SkillRegistry>>, Arc<RwLock<SkillManager>>, Arc<SkillExecutor>) {
    let loader = SkillLoader::new(SkillLoader::default_dirs());
    let skills = loader.load_all().unwrap_or_default();
    let registry = Arc::new(RwLock::new(SkillRegistry::new()));
    let reg: Arc<RwLock<SkillRegistry>> = Arc::clone(&registry);
    for skill in skills {
        if let Err(e) = reg.write().register(skill) {
            tracing::warn!("Failed to register skill: {}", e);
        }
    }

    let manager = SkillManager::new().unwrap_or_else(|_| {
        tracing::warn!("Failed to create SkillManager, using temp dir");
        SkillManager::with_dir(std::env::temp_dir().join("hermes-skills"))
    });
    let manager = Arc::new(RwLock::new(manager));

    let executor = SkillExecutor::from_default_dirs()
        .unwrap_or_else(|_| SkillExecutor::new(Arc::clone(&registry)));

    (registry, manager, Arc::new(executor))
}
