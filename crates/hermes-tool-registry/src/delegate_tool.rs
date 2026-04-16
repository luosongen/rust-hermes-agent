//! DelegateTool — spawns subagents to handle tasks in parallel.

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

/// DelegateTool allows the agent to spawn subagents with restricted toolsets.
pub struct DelegateTool {
    parent_agent: Arc<Agent>,
    max_concurrent: usize,
    max_depth: u8,
}

impl DelegateTool {
    pub fn new(parent_agent: Arc<Agent>) -> Self {
        Self {
            parent_agent,
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            max_depth: MAX_DELEGATION_DEPTH,
        }
    }

    pub fn with_config(parent_agent: Arc<Agent>, max_concurrent: usize, max_depth: u8) -> Self {
        Self {
            parent_agent,
            max_concurrent,
            max_depth,
        }
    }

    /// Filter tool definitions by name prefix and strip blocked tools.
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

    fn build_child_prompt(goal: &str, context: Option<&str>) -> String {
        let context_str = context.unwrap_or("");
        format!(
            "You are a subagent. Your task:\n\n## Goal\n{}\n\n## Context\n{}\n\nProvide a structured summary of what you accomplished: actions taken, findings, files created/modified, and any issues.",
            goal,
            context_str
        )
    }

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
