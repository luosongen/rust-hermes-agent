//! Agent 主循环模块
//!
//! 本模块是 hermes-core 的核心——实现了 Agent 的主体逻辑，即与 LLM 交互、
//! 调度工具、处理响应的主循环。
//!
//! ## 主要类型
//! - **AgentConfig**: Agent 配置，包含最大迭代次数、模型、温度、最大 token 数和工作目录
//! - **Agent**: 主 Agent 结构体，持有一个 LLM Provider、一个工具调度器和一个会话存储
//!
//! ## Agent 循环逻辑
//! 1. 从会话存储加载历史消息（或创建空消息列表）
//! 2. 将用户输入追加到消息列表
//! 3. 循环迭代（最多 max_iterations 次）:
//!    a. 调用 LLM Provider 的 `chat()` 发送请求
//!    b. 若 LLM 返回工具调用，则执行工具并追加结果，继续循环
//!    c. 若 LLM 返回停止，则保存到会话存储并返回响应
//!    d. 处理各种 `FinishReason`（长度超出、内容过滤、未知）
//!
//! ## 与其他模块的关系
//! - 依赖 `LlmProvider`（`provider.rs`）进行 LLM 调用
//! - 依赖 `ToolDispatcher`（`tool_dispatcher.rs`）执行工具
//! - 依赖 `SessionStore`（`hermes-memory`）持久化会话历史
//! - 定义了 `AgentError` 相关的错误转换

use crate::{
    AgentError, ChatRequest, ConversationRequest, ConversationResponse, LlmProvider, ModelId,
    NudgeConfig, NudgeService, NudgeState, NudgeTrigger, Role, ToolContext, ToolDispatcher,
};
use hermes_memory::{NewMessage, SessionStore};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::SystemTime;

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_iterations: usize,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
    pub working_directory: std::path::PathBuf,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 90,
            model: "openai/gpt-4o".to_string(),
            temperature: None,
            max_tokens: None,
            working_directory: std::env::current_dir().unwrap_or_else(|_| ".".into()),
        }
    }
}

/// Agent — main agentic loop
pub struct Agent {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<dyn ToolDispatcher>,
    session_store: Arc<dyn SessionStore>,
    config: AgentConfig,
    // Nudge system
    nudge_service: Arc<NudgeService>,
    nudge_state: Arc<Mutex<NudgeState>>,
}

impl Agent {
    /// Returns a reference to the LLM provider.
    pub fn provider(&self) -> Arc<dyn LlmProvider> {
        Arc::clone(&self.provider)
    }

    /// Returns a reference to the agent config.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Returns a reference to the tool dispatcher.
    pub fn tools(&self) -> Arc<dyn ToolDispatcher> {
        Arc::clone(&self.tools)
    }

    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
        nudge_config: NudgeConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            session_store,
            config,
            nudge_service: Arc::new(NudgeService::new(nudge_config)),
            nudge_state: Arc::new(Mutex::new(NudgeState::default())),
        }
    }

    /// Create Agent with nudge disabled (for subagents to prevent nested nudges)
    pub fn new_with_nudge_disabled(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
    ) -> Self {
        Self::new(
            provider,
            tools,
            session_store,
            config,
            NudgeConfig::disabled(),
        )
    }

    /// Run a conversation
    pub async fn run_conversation(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AgentError> {
        let messages = if let Some(session_id) = &request.session_id {
            let msgs = self
                .session_store
                .get_messages(session_id, 1000, 0)
                .await?;
            msgs.into_iter()
                .map(|m| crate::Message {
                    role: match m.role.as_str() {
                        "system" => Role::System,
                        "user" => Role::User,
                        "assistant" => Role::Assistant,
                        "tool" => Role::Tool,
                        _ => Role::User,
                    },
                    content: crate::Content::Text(m.content.unwrap_or_default()),
                    reasoning: m.reasoning,
                    tool_call_id: m.tool_call_id,
                    tool_name: m.tool_name,
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut messages = messages;
        messages.push(crate::Message::user(request.content.clone()));

        let mut iterations = 0;

        loop {
            if iterations >= self.config.max_iterations {
                return Err(AgentError::IterationExhausted);
            }

            let model_id = ModelId::parse(&self.config.model)
                .unwrap_or_else(|| ModelId::new("openai", "gpt-4o"));

            let chat_request = ChatRequest {
                model: model_id.clone(),
                messages: messages.clone(),
                tools: Some(self.tools.get_definitions()),
                system_prompt: request.system_prompt.clone(),
                temperature: self.config.temperature,
                max_tokens: self.config.max_tokens,
            };

            // 使用智能路由
            let response = self
                .provider
                .chat(chat_request)
                .await
                .map_err(AgentError::Provider)?;

            match response.finish_reason {
                crate::FinishReason::Stop => {
                    if let Some(tool_calls) = response.tool_calls {
                        for call in &tool_calls {
                            let context = ToolContext {
                                session_id: request
                                    .session_id
                                    .clone()
                                    .unwrap_or_default(),
                                working_directory: self.config.working_directory.clone(),
                                user_id: None,
                                task_id: Some(call.id.clone()),
                            };
                            let result = self
                                .tools
                                .dispatch(call, context)
                                .await
                                .map_err(AgentError::Tool)?;
                            messages.push(crate::Message::tool_result(
                                call.id.clone(),
                                crate::Content::Text(result),
                            ));
                        }
                        // Track tool calls for skill nudge
                        self.nudge_state.lock().iters_since_skill += tool_calls.len();
                        iterations += 1;
                        continue;
                    }

                    // ========== Nudge: Check triggers ==========
                    let trigger = {
                        let mut nudge_state = self.nudge_state.lock();
                        self.nudge_service.check_triggers(
                            &mut nudge_state,
                            messages.len(),
                            0,  // no tool calls this turn
                        )
                    };

                    if trigger != NudgeTrigger::None {
                        let prompt = self.nudge_service.get_prompt(trigger);

                        // Spawn background review (fire-and-forget)
                        // Clone only Send+Sync types for the background task
                        let provider = self.provider.clone();
                        let messages_for_review = messages.clone();

                        tokio::spawn(async move {
                            // Simple review: just send a single chat request
                            let model_id = ModelId::parse("openai/gpt-4o")
                                .unwrap_or_else(|| ModelId::new("openai", "gpt-4o"));

                            let chat_request = ChatRequest {
                                model: model_id,
                                messages: messages_for_review,
                                tools: None,  // Review agent has no tools
                                system_prompt: Some(prompt.to_string()),
                                temperature: None,
                                max_tokens: None,
                            };

                            if let Err(e) = provider.chat(chat_request).await {
                                tracing::debug!("Background review failed: {}", e);
                            }
                        });
                    }

                    if let Some(session_id) = &request.session_id {
                        let now = SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64();
                        let _ = self
                            .session_store
                            .append_message(
                                session_id,
                                NewMessage {
                                    role: "assistant".to_string(),
                                    content: Some(response.content.clone()),
                                    tool_call_id: None,
                                    tool_calls: None,
                                    tool_name: None,
                                    timestamp: now,
                                    token_count: response
                                        .usage
                                        .as_ref()
                                        .map(|u| u.output_tokens),
                                    finish_reason: Some("stop".to_string()),
                                    reasoning: response.reasoning.clone(),
                                },
                            )
                            .await;
                    }

                    return Ok(ConversationResponse {
                        content: response.content,
                        session_id: request.session_id,
                        usage: response.usage,
                    });
                }
                crate::FinishReason::Length => {
                    // 尝试使用上下文压缩
                    let mut compressor = crate::ContextCompressor::new(
                        self.provider.clone(),
                        self.config.model.clone(),
                        self.provider.context_length(&model_id).unwrap_or(4096),
                    );
                    
                    match compressor.compress(messages.clone(), None, None).await {
                        Ok(compressed_messages) => {
                            messages = compressed_messages;
                            continue;
                        }
                        Err(e) => {
                            return Err(AgentError::Internal(
                                format!("Context length exceeded and compression failed: {}", e),
                            ));
                        }
                    }
                }
                crate::FinishReason::ContentFilter => {
                    return Err(AgentError::ContentFiltered);
                }
                crate::FinishReason::Other => {
                    return Err(AgentError::UnknownFinishReason);
                }
            }
        }
    }
}
