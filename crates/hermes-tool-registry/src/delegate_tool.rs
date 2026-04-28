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
//! - **进度报告** — 通过 broadcast 通道报告任务进度
//! - **超时控制** — 支持任务超时自动取消

use async_trait::async_trait;
use hermes_core::{
    Agent, AgentConfig, ChatRequest, ConversationResponse, FinishReason,
    Message, ModelId, ToolContext, ToolDefinition, ToolError,
};
use hermes_core::delegate::types::{
    BatchDelegateResult, DelegateParams, DelegateProgress, DelegateProgressStatus,
    DelegateResult, DelegateStatus, DelegateTask,
    DEFAULT_MAX_CONCURRENT, DEFAULT_TIMEOUT_SECONDS, BLOCKED_TOOLS, MAX_DELEGATION_DEPTH,
    ProgressSender,
};
use crate::Tool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

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
    /// 任务超时时间（秒）
    timeout_seconds: u64,
    /// 进度报告发送器（可选）
    progress_sender: Option<ProgressSender>,
}

impl DelegateTool {
    /// 使用默认配置创建委托工具
    pub fn new(parent_agent: Arc<Agent>) -> Self {
        Self {
            parent_agent,
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            max_depth: MAX_DELEGATION_DEPTH,
            timeout_seconds: DEFAULT_TIMEOUT_SECONDS,
            progress_sender: None,
        }
    }

    /// 使用自定义配置创建委托工具
    pub fn with_config(
        parent_agent: Arc<Agent>,
        max_concurrent: usize,
        max_depth: u8,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            parent_agent,
            max_concurrent,
            max_depth,
            timeout_seconds,
            progress_sender: None,
        }
    }

    /// 设置进度报告发送器
    pub fn with_progress_sender(mut self, sender: ProgressSender) -> Self {
        self.progress_sender = Some(sender);
        self
    }

    /// 发送进度报告
    fn send_progress(&self, progress: DelegateProgress) {
        if let Some(sender) = &self.progress_sender {
            let _ = sender.send(progress);
        }
    }

    /// 根据 toolsets 白名单过滤工具定义
    fn filter_tools(&self, toolsets: Option<&[String]>) -> Option<Vec<ToolDefinition>> {
        let all_tools = self.parent_agent.tools().get_definitions();

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

    /// 生成任务 ID
    fn generate_task_id() -> String {
        Uuid::new_v4().to_string()[..8].to_string()
    }

    /// 运行单个子 Agent 任务
    async fn run_single_child(&self, task: DelegateTask, parent_depth: u8) -> DelegateResult {
        let start = Instant::now();
        let task_id = task.task_id.clone().unwrap_or_else(|| Self::generate_task_id());

        // 发送开始进度
        self.send_progress(DelegateProgress {
            task_id: task_id.clone(),
            status: DelegateProgressStatus::Pending,
            message: "任务开始".to_string(),
            percentage: Some(0),
            tool_calls: 0,
            api_calls: 0,
            elapsed_ms: 0,
        });

        if parent_depth >= self.max_depth {
            return DelegateResult {
                status: DelegateStatus::Error,
                summary: format!("Max delegation depth ({}) exceeded", self.max_depth),
                api_calls: 0,
                duration_ms: start.elapsed().as_millis() as u64,
                model: String::new(),
                exit_reason: "depth_exceeded".to_string(),
                tool_trace: vec![],
                task_id: Some(task_id),
            };
        }

        // 使用 tokio::time::timeout 包装任务
        let timeout_duration = Duration::from_secs(self.timeout_seconds);
        let child_prompt = Self::build_child_prompt(&task.goal, task.context.as_deref());
        let child_config = AgentConfig {
            max_iterations: task.max_iterations as usize,
            model: self.parent_agent.config().model.clone(),
            temperature: self.parent_agent.config().temperature,
            max_tokens: self.parent_agent.config().max_tokens,
            working_directory: self.parent_agent.config().working_directory.clone(),
            yolo_mode: self.parent_agent.config().yolo_mode,
            checkpoint_manager: self.parent_agent.config().checkpoint_manager.clone(),
        };
        let tools = self.filter_tools(task.toolsets.as_deref());

        let result = tokio::time::timeout(
            timeout_duration,
            self.spawn_child_agent(&child_config, &child_prompt, parent_depth + 1, tools, &task_id),
        ).await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok((response, api_calls))) => {
                self.send_progress(DelegateProgress {
                    task_id: task_id.clone(),
                    status: DelegateProgressStatus::Completed,
                    message: "任务完成".to_string(),
                    percentage: Some(100),
                    tool_calls: 0,
                    api_calls,
                    elapsed_ms: duration_ms,
                });

                DelegateResult {
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
                    task_id: Some(task_id),
                }
            }
            Ok(Err(e)) => {
                self.send_progress(DelegateProgress {
                    task_id: task_id.clone(),
                    status: DelegateProgressStatus::Failed,
                    message: format!("任务失败: {}", e),
                    percentage: None,
                    tool_calls: 0,
                    api_calls: 0,
                    elapsed_ms: duration_ms,
                });

                DelegateResult {
                    status: DelegateStatus::Error,
                    summary: e.to_string(),
                    api_calls: 0,
                    duration_ms,
                    model: self.parent_agent.config().model.clone(),
                    exit_reason: "error".to_string(),
                    tool_trace: vec![],
                    task_id: Some(task_id),
                }
            }
            Err(_) => {
                self.send_progress(DelegateProgress {
                    task_id: task_id.clone(),
                    status: DelegateProgressStatus::Timeout,
                    message: format!("任务超时 ({}秒)", self.timeout_seconds),
                    percentage: None,
                    tool_calls: 0,
                    api_calls: 0,
                    elapsed_ms: duration_ms,
                });

                DelegateResult {
                    status: DelegateStatus::Timeout,
                    summary: format!("Task timed out after {} seconds", self.timeout_seconds),
                    api_calls: 0,
                    duration_ms,
                    model: self.parent_agent.config().model.clone(),
                    exit_reason: "timeout".to_string(),
                    tool_trace: vec![],
                    task_id: Some(task_id),
                }
            }
        }
    }

    /// 构建子 Agent 的系统提示词
    fn build_child_prompt(goal: &str, context: Option<&str>) -> String {
        let context_str = context.unwrap_or("");
        format!(
            "You are a subagent. Your task:\n\n## Goal\n{}\n\n## Context\n{}\n\nProvide a structured summary of what you accomplished: actions taken, findings, files created/modified, and any issues.",
            goal,
            context_str
        )
    }

    /// 生成并运行一个子 Agent
    async fn spawn_child_agent(
        &self,
        config: &AgentConfig,
        system_prompt: &str,
        _depth: u8,
        tools: Option<Vec<ToolDefinition>>,
        task_id: &str,
    ) -> Result<(ConversationResponse, u32), ToolError> {
        let mut messages = vec![Message::user(system_prompt.to_string())];
        let mut api_calls = 0u32;
        let start = Instant::now();

        let mut iterations = 0;
        loop {
            if iterations >= config.max_iterations {
                break;
            }

            // 计算进度百分比
            let progress_pct = ((iterations as f32 / config.max_iterations as f32) * 100.0) as u8;
            let elapsed_ms = start.elapsed().as_millis() as u64;

            // 发送运行进度
            self.send_progress(DelegateProgress {
                task_id: task_id.to_string(),
                status: DelegateProgressStatus::Running,
                message: format!("迭代 {} / {}", iterations + 1, config.max_iterations),
                percentage: Some(progress_pct),
                tool_calls: 0,
                api_calls,
                elapsed_ms,
            });

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
            timeout_seconds: self.timeout_seconds,
            progress_sender: self.progress_sender.clone(),
        }
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate"
    }

    fn description(&self) -> &str {
        "Delegate a task to subagent(s) that run in parallel with restricted toolsets. Supports timeout and progress reporting."
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
                        "max_iterations": { "type": "integer", "default": 50 },
                        "task_id": { "type": "string", "description": "Optional task ID for progress tracking" }
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
                                    "max_iterations": { "type": "integer" },
                                    "task_id": { "type": "string" }
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
            let batch_result = BatchDelegateResult::new(results);
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
                        task_id: params.task_id,
                    },
                    0,
                )
                .await;
            serde_json::to_string(&result).map_err(|e| ToolError::Execution(e.to_string()))
        }
    }
}
