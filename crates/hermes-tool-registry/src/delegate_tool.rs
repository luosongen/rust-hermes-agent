//! DelegateTool — 委托工具，允许 Agent 生成子 Agent 并行处理任务
//!
//! 本模块实现了 `Tool` trait，提供 `delegate` 工具，使主 Agent 能够将复杂任务
//! 委托给一个或多个子 Agent 并行执行。子 Agent 拥有受限的工具集，从而实现安全隔离。
//!
//! ## 核心功能
//! - **单任务委托** — 通过 `goal` 参数将单个任务委托给一个子 Agent
//! - **批量并行委托** — 通过 `tasks` 数组将多个任务并行分配给多个子 Agent
//! - **工具集过滤** — 子 Agent 仅能使用 `toolsets` 白名单中指定的工具
//! - **深度限制** — 防止无限递归委托，最大深度由 `MAX_DELEGATION_DEPTH` 控制
//! - **并发限制** — 批量委托通过信号量控制最大并发数
//!
//! ## 与 Agent 的关系
//! - 子 Agent 与父 Agent 共享相同的 LLM Provider
//! - 子 Agent 的模型、温度、最大 token 等配置继承自父 Agent
//! - 子 Agent 无法使用 `delegate` 工具本身（通过 `BLOCKED_TOOLS` 黑名单过滤）

use async_trait::async_trait;
use hermes_core::{
    Agent, AgentConfig, ChatRequest, ConversationResponse, FinishReason,
    Message, ModelId, ToolContext, ToolDefinition, ToolError,
};
use hermes_core::delegate::types::{
    BatchDelegateResult, DelegateParams, DelegateResult, DelegateStatus, DelegateTask,
    DEFAULT_MAX_CONCURRENT, BLOCKED_TOOLS, MAX_DELEGATION_DEPTH,
};
use crate::Tool;
use std::sync::Arc;
use std::time::Instant;

/// 委托工具 — 允许 Agent 生成子 Agent 并行处理任务
///
/// 持有父 Agent 的引用，通过受限工具集和深度限制来隔离子 Agent 的能力。
/// 子 Agent 使用与父 Agent 相同的 LLM Provider 进行对话。
pub struct DelegateTool {
    /// 父 Agent 的引用，用于生成子 Agent 和获取其配置
    parent_agent: Arc<Agent>,
    /// 批量委托时的最大并发数
    max_concurrent: usize,
    /// 允许的最大委托深度
    max_depth: u8,
}

impl DelegateTool {
    /// 使用默认配置（`DEFAULT_MAX_CONCURRENT` 和 `MAX_DELEGATION_DEPTH`）创建委托工具
    pub fn new(parent_agent: Arc<Agent>) -> Self {
        Self {
            parent_agent,
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            max_depth: MAX_DELEGATION_DEPTH,
        }
    }

    /// 使用自定义配置创建委托工具
    ///
    /// - `parent_agent` — 父 Agent，用于生成子 Agent
    /// - `max_concurrent` — 批量委托时的最大并发数
    /// - `max_depth` — 允许的最大委托深度
    pub fn with_config(parent_agent: Arc<Agent>, max_concurrent: usize, max_depth: u8) -> Self {
        Self {
            parent_agent,
            max_concurrent,
            max_depth,
        }
    }

    /// 根据 toolsets 白名单过滤工具定义，并移除被 BLOCKED_TOOLS 黑名单禁止的工具
    ///
    /// 如果未指定 toolsets，则返回所有非黑名单工具。如果过滤后结果为空，返回 `None`。
    fn filter_tools(&self, toolsets: Option<&[String]>) -> Option<Vec<ToolDefinition>> {
        let all_tools = self.parent_agent.tools().get_definitions();

        // If no toolsets specified, use all non-blocked tools
        let filtered: Vec<ToolDefinition> = if let Some(names) = toolsets {
            all_tools
                .into_iter()
                .filter(|t| names.iter().any(|n| t.name == *n || t.name.starts_with(&format!("{}.", n))))
                .filter(|t| !BLOCKED_TOOLS.iter().any(|&b| t.name == b || t.name.starts_with(&format!("{}.", b))))
                .collect()
        } else {
            all_tools
                .into_iter()
                .filter(|t| !BLOCKED_TOOLS.iter().any(|&b| t.name == b || t.name.starts_with(&format!("{}.", b))))
                .collect()
        };

        if filtered.is_empty() {
            None
        } else {
            Some(filtered)
        }
    }

    /// 运行单个子 Agent 任务
    ///
    /// 检查深度限制，构建子 Agent 的提示词和配置，过滤工具集，然后生成并等待子 Agent 完成。
    /// 返回 `DelegateResult`，包含执行状态、摘要、API 调用次数和耗时。
    async fn run_single_child(&self, task: DelegateTask, parent_depth: u8) -> DelegateResult {
        let start = Instant::now();

        if parent_depth >= self.max_depth {
            return DelegateResult {
                status: DelegateStatus::Error,
                summary: format!("Max delegation depth ({}) exceeded", self.max_depth),
                api_calls: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                model: String::new(),
                exit_reason: "depth_exceeded".to_string(),
                tool_trace: vec![],
            };
        }

        let child_prompt =
            Self::build_child_prompt(&task.goal, task.context.as_deref());
        let child_config = AgentConfig {
            max_iterations: task.max_iterations as usize,
            model: self.parent_agent.config().model.clone(),
            temperature: self.parent_agent.config().temperature,
            max_tokens: self.parent_agent.config().max_tokens,
            working_directory: self.parent_agent.config().working_directory.clone(),
        };

        let tools = self.filter_tools(task.toolsets.as_deref());

        let result = self
            .spawn_child_agent(&child_config, &child_prompt, parent_depth + 1, tools)
            .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok((response, api_calls)) => DelegateResult {
                status: if response.content.is_empty() {
                    DelegateStatus::Failed
                } else {
                    DelegateStatus::Completed
                },
                summary: response.content,
                api_calls,
                duration_ms,
                model: self.parent_agent.config().model.clone(),
                exit_reason: "completed".to_string(),
                tool_trace: vec![],
            },
            Err(e) => DelegateResult {
                status: DelegateStatus::Error,
                summary: e.to_string(),
                api_calls: 0,
                duration_ms,
                model: self.parent_agent.config().model.clone(),
                exit_reason: "error".to_string(),
                tool_trace: vec![],
            },
        }
    }

    /// 构建子 Agent 的系统提示词
    ///
    /// 将目标（goal）和上下文（context）格式化为一个结构化的提示词，
    /// 告知子 Agent 其角色和任务，并要求返回结构化的执行摘要。
    fn build_child_prompt(goal: &str, context: Option<&str>) -> String {
        let context_str = context.unwrap_or("");
        format!(
            "You are a subagent. Your task:\n\n## Goal\n{}\n\n## Context\n{}\n\nProvide a structured summary of what you accomplished: actions taken, findings, files created/modified, and any issues.",
            goal,
            context_str
        )
    }

    /// 生成并运行一个子 Agent
    ///
    /// 循环调用 LLM Provider，传入过滤后的工具集，直到达到停止条件（finish_reason == Stop）
    /// 或达到最大迭代次数。记录 API 调用次数并返回对话响应。
    async fn spawn_child_agent(
        &self,
        config: &AgentConfig,
        system_prompt: &str,
        _depth: u8,
        tools: Option<Vec<ToolDefinition>>,
    ) -> Result<(ConversationResponse, u32), ToolError> {
        let mut messages = vec![Message::user(system_prompt.to_string())];
        let mut api_calls = 0u32;

        let mut iterations = 0;
        loop {
            if iterations >= config.max_iterations {
                break;
            }

            let model_id =
                ModelId::parse(&config.model).unwrap_or_else(|| ModelId::new("openai", "gpt-4o"));

            let chat_request = ChatRequest {
                model: model_id,
                messages: messages.clone(),
                tools: tools.clone(),
                system_prompt: None,
                temperature: config.temperature,
                max_tokens: config.max_tokens,
            };

            let response = self
                .parent_agent
                .provider()
                .chat(chat_request)
                .await
                .map_err(|e| ToolError::Execution(e.to_string()))?;

            api_calls += 1;
            messages.push(Message::assistant(response.content.clone()));

            if response.finish_reason == FinishReason::Stop {
                return Ok((
                    ConversationResponse {
                        content: response.content,
                        session_id: None,
                        usage: response.usage,
                    },
                    api_calls,
                ));
            }

            iterations += 1;
        }

        Err(ToolError::Execution("Max iterations exceeded".to_string()))
    }

    /// 批量并行运行多个子 Agent 任务
    ///
    /// 使用 tokio 信号量（Semaphore）限制最大并发数，将任务分配给独立的异步任务，
    /// 等待所有任务完成后返回 `Vec<DelegateResult>`。
    async fn run_batch(
        &self,
        tasks: Vec<DelegateTask>,
        max_concurrent: usize,
        parent_depth: u8,
    ) -> Vec<DelegateResult> {
        use tokio::sync::Semaphore;

        let sem = Arc::new(Semaphore::new(max_concurrent));
        let mut handles = Vec::new();

        for task in tasks {
            let sem = sem.clone();
            let tool = self.clone();
            let depth = parent_depth;

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire_owned().await
                    .map_err(|_| ToolError::Execution("Semaphore closed".into())).unwrap();
                tool.run_single_child(task, depth).await
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            results.push(handle.await.unwrap());
        }
        results
    }
}

impl Clone for DelegateTool {
    fn clone(&self) -> Self {
        Self {
            parent_agent: Arc::clone(&self.parent_agent),
            max_concurrent: self.max_concurrent,
            max_depth: self.max_depth,
        }
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate"
    }

    fn description(&self) -> &str {
        "Delegate a task to subagent(s) that run in parallel with restricted toolsets"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "goal": { "type": "string", "description": "Single task goal" },
                        "context": { "type": "string", "description": "Background context" },
                        "toolsets": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Toolset whitelist for subagent"
                        },
                        "max_iterations": { "type": "integer", "default": 50 }
                    },
                    "required": ["goal"]
                },
                {
                    "properties": {
                        "tasks": {
                            "type": "array",
                            "description": "Batch of tasks to run in parallel",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "goal": { "type": "string" },
                                    "context": { "type": "string" },
                                    "toolsets": { "type": "array", "items": { "type": "string" } },
                                    "max_iterations": { "type": "integer" }
                                },
                                "required": ["goal"]
                            }
                        },
                        "max_concurrent": { "type": "integer", "default": 3 }
                    },
                    "required": ["tasks"]
                }
            ]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        if let Some(tasks) = args.get("tasks").and_then(|t| t.as_array()) {
            let tasks: Vec<DelegateTask> = tasks
                .iter()
                .map(|t| serde_json::from_value(t.clone()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

            let max_concurrent = args
                .get("max_concurrent")
                .and_then(|v| v.as_u64())
                .unwrap_or(DEFAULT_MAX_CONCURRENT as u64) as usize;

            let results = self.run_batch(tasks, max_concurrent, 0).await;
            let batch_result = BatchDelegateResult {
                total_duration_ms: results.iter().map(|r| r.duration_ms).sum(),
                results,
            };
            serde_json::to_string(&batch_result).map_err(|e| ToolError::Execution(e.to_string()))
        } else {
            let params: DelegateParams = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let result = self
                .run_single_child(
                    DelegateTask {
                        goal: params.goal,
                        context: params.context,
                        toolsets: params.toolsets,
                        max_iterations: params.max_iterations,
                    },
                    0,
                )
                .await;
            serde_json::to_string(&result).map_err(|e| ToolError::Execution(e.to_string()))
        }
    }
}
