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
    AgentError, ChatRequest, ConversationRequest, ConversationResponse, DisplayHandler,
    LlmProvider, ModelId, NudgeConfig, NudgeService, NudgeState, NudgeTrigger, Role,
    TitleGenerator, ToolContext, ToolDispatcher, TrajectorySaver, RetryConfig, ContextCompressor,
};
use crate::insights::{InsightsTracker, ToolCallRecord};
use crate::rate_limit_tracker::RateLimitTracker;
use crate::retry_utils::with_retry;
use crate::usage_pricing::{CostCalculator, PricingDatabase};
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
    /// YOLO 模式 — 跳过危险命令审批检查
    pub yolo_mode: bool,
    /// 文件检查点管理器
    pub checkpoint_manager: Option<std::sync::Arc<hermes_checkpoint::CheckpointManager>>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 90,
            model: "openai/gpt-4o".to_string(),
            temperature: None,
            max_tokens: None,
            working_directory: std::env::current_dir().unwrap_or_else(|_| ".".into()),
            yolo_mode: false,
            checkpoint_manager: None,
        }
    }
}

/// Agent — main agentic loop
pub struct Agent {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<dyn ToolDispatcher>,
    session_store: Arc<dyn SessionStore>,
    /// Agent 配置（公开以允许 CLI 层在运行时更新 YOLO/Fast 模式）
    pub config: AgentConfig,
    // Nudge system
    nudge_service: Arc<NudgeService>,
    nudge_state: Arc<Mutex<NudgeState>>,
    // Display handler
    display_handler: Option<Arc<dyn DisplayHandler>>,
    // Title generator
    title_generator: Option<Arc<TitleGenerator>>,
    // Trajectory saver
    trajectory_saver: Option<TrajectorySaver>,
    // Insights tracker
    insights_tracker: Option<Arc<dyn InsightsTracker>>,
    // Rate limit tracker
    rate_limit_tracker: Option<Arc<RateLimitTracker>>,
    // Retry config
    retry_config: RetryConfig,
    // 上下文压缩器（跨迭代持久化状态）
    context_compressor: Option<ContextCompressor>,
}

/// Agent 构建器，使用流式 builder 模式替代 11 参数的构造函数
///
/// # 示例
/// ```ignore
/// let agent = AgentBuilder::new()
///     .provider(provider)
///     .tools(tools)
///     .session_store(store)
///     .config(AgentConfig::default())
///     .build();
/// ```
// 注意：由于 trait object 和某些类型不支持 Debug/Clone，这里手动实现 Debug 并省略 Clone
pub struct AgentBuilder {
    provider: Option<Arc<dyn LlmProvider>>,
    tools: Option<Arc<dyn ToolDispatcher>>,
    session_store: Option<Arc<dyn SessionStore>>,
    config: Option<AgentConfig>,
    nudge_config: NudgeConfig,
    display_handler: Option<Arc<dyn DisplayHandler>>,
    title_generator: Option<Arc<TitleGenerator>>,
    trajectory_saver: Option<TrajectorySaver>,
    insights_tracker: Option<Arc<dyn InsightsTracker>>,
    rate_limit_tracker: Option<Arc<RateLimitTracker>>,
    retry_config: RetryConfig,
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self {
            provider: None,
            tools: None,
            session_store: None,
            config: None,
            nudge_config: NudgeConfig::default(),
            display_handler: None,
            title_generator: None,
            trajectory_saver: None,
            insights_tracker: None,
            rate_limit_tracker: None,
            retry_config: RetryConfig::default(),
        }
    }
}

impl std::fmt::Debug for AgentBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentBuilder")
            .field("provider", &self.provider.is_some())
            .field("tools", &self.tools.is_some())
            .field("session_store", &self.session_store.is_some())
            .field("config", &self.config)
            .field("nudge_config", &self.nudge_config)
            .field("display_handler", &self.display_handler.is_some())
            .field("title_generator", &self.title_generator.is_some())
            .field("trajectory_saver", &self.trajectory_saver.is_some())
            .field("insights_tracker", &self.insights_tracker.is_some())
            .field("rate_limit_tracker", &self.rate_limit_tracker.is_some())
            .field("retry_config", &self.retry_config)
            .finish()
    }
}

impl AgentBuilder {
    /// 创建新的 AgentBuilder
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 LLM provider（必需）
    pub fn provider(mut self, provider: Arc<dyn LlmProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// 设置工具调度器（必需）
    pub fn tools(mut self, tools: Arc<dyn ToolDispatcher>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// 设置会话存储（必需）
    pub fn session_store(mut self, session_store: Arc<dyn SessionStore>) -> Self {
        self.session_store = Some(session_store);
        self
    }

    /// 设置 Agent 配置（必需）
    pub fn config(mut self, config: AgentConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// 设置 Nudge 配置（可选，默认启用）
    pub fn nudge_config(mut self, nudge_config: NudgeConfig) -> Self {
        self.nudge_config = nudge_config;
        self
    }

    /// 设置显示处理器（可选）
    pub fn display_handler(mut self, display_handler: Option<Arc<dyn DisplayHandler>>) -> Self {
        self.display_handler = display_handler;
        self
    }

    /// 设置标题生成器（可选）
    pub fn title_generator(mut self, title_generator: Option<Arc<TitleGenerator>>) -> Self {
        self.title_generator = title_generator;
        self
    }

    /// 设置轨迹保存器（可选）
    pub fn trajectory_saver(mut self, trajectory_saver: Option<TrajectorySaver>) -> Self {
        self.trajectory_saver = trajectory_saver;
        self
    }

    /// 设置洞察追踪器（可选）
    pub fn insights_tracker(mut self, insights_tracker: Option<Arc<dyn InsightsTracker>>) -> Self {
        self.insights_tracker = insights_tracker;
        self
    }

    /// 设置速率限制追踪器（可选）
    pub fn rate_limit_tracker(mut self, rate_limit_tracker: Option<Arc<RateLimitTracker>>) -> Self {
        self.rate_limit_tracker = rate_limit_tracker;
        self
    }

    /// 设置重试配置（可选）
    pub fn retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// 构建 Agent 实例
    ///
    /// # Panics
    /// 如果必需字段（provider, tools, session_store, config）未设置，则 panic
    pub fn build(self) -> Agent {
        Agent::new(
            self.provider.expect("provider is required"),
            self.tools.expect("tools is required"),
            self.session_store.expect("session_store is required"),
            self.config.expect("config is required"),
            self.nudge_config,
            self.display_handler,
            self.title_generator,
            self.trajectory_saver,
            self.insights_tracker,
            self.rate_limit_tracker,
            self.retry_config,
        )
    }
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
        display_handler: Option<Arc<dyn DisplayHandler>>,
        title_generator: Option<Arc<TitleGenerator>>,
        trajectory_saver: Option<TrajectorySaver>,
        insights_tracker: Option<Arc<dyn InsightsTracker>>,
        rate_limit_tracker: Option<Arc<RateLimitTracker>>,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            session_store,
            config,
            nudge_service: Arc::new(NudgeService::new(nudge_config)),
            nudge_state: Arc::new(Mutex::new(NudgeState::default())),
            display_handler,
            title_generator,
            trajectory_saver,
            insights_tracker,
            rate_limit_tracker,
            retry_config,
            context_compressor: None,
        }
    }

    /// Create Agent with nudge disabled (for subagents to prevent nested nudges)
    pub fn new_with_nudge_disabled(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
        display_handler: Option<Arc<dyn DisplayHandler>>,
        title_generator: Option<Arc<TitleGenerator>>,
        trajectory_saver: Option<TrajectorySaver>,
        insights_tracker: Option<Arc<dyn InsightsTracker>>,
        rate_limit_tracker: Option<Arc<RateLimitTracker>>,
        retry_config: RetryConfig,
    ) -> Self {
        Self::new(
            provider,
            tools,
            session_store,
            config,
            NudgeConfig::disabled(),
            display_handler,
            title_generator,
            trajectory_saver,
            insights_tracker,
            rate_limit_tracker,
            retry_config,
        )
    }

    /// Run a conversation
    pub async fn run_conversation(
        &mut self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AgentError> {
        let messages = if let Some(session_id) = &request.session_id {
            let msgs = self.session_store.get_messages(session_id, 1000, 0).await?;
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
        let mut final_result: Result<ConversationResponse, AgentError> = Err(AgentError::IterationExhausted);

        loop {
            if iterations >= self.config.max_iterations {
                final_result = Err(AgentError::IterationExhausted);
                break;
            }

            let model_id = ModelId::parse(&self.config.model)
                .unwrap_or_else(|| ModelId::new("openai", "gpt-4o"));

            // ========== Context Pressure Monitoring ==========
            let context_length = self.provider.context_length(&model_id).unwrap_or(100_000);
            let prompt_tokens: usize = messages
                .iter()
                .map(|m| match &m.content {
                    crate::Content::Text(t) => self.provider.estimate_tokens(t, &model_id),
                    crate::Content::Image { .. } => 50,
                    crate::Content::ToolResult { content, .. } => self.provider.estimate_tokens(content, &model_id),
                })
                .sum();

            let monitor = crate::ContextPressureMonitor::new(context_length);

            // Proactive compression check (at critical level before first iteration)
            // 使用 get_or_insert_with 模式复用已存在的压缩器，保持状态持久化
            if monitor.should_compress(prompt_tokens) && iterations == 0 {
                let compressor = self.context_compressor.get_or_insert_with(|| {
                    crate::ContextCompressor::new(
                        self.provider.clone(),
                        self.config.model.clone(),
                        context_length,
                    )
                });
                match compressor.compress(messages.clone(), None, None).await {
                    Ok(compressed) => {
                        tracing::info!("Context compressed proactively due to high memory pressure ({} tokens)", prompt_tokens);
                        messages = compressed;
                    }
                    Err(e) => {
                        tracing::debug!("Proactive compression failed: {}", e);
                    }
                }
            }

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
                            // Display: tool started
                            if let Some(display) = &self.display_handler {
                                let args_value = serde_json::to_value(&call.arguments).unwrap_or_default();
                                display.tool_started(&call.name, &args_value);
                                display.flush();
                            }

                            let context = ToolContext {
                                session_id: request.session_id.clone().unwrap_or_default(),
                                working_directory: self.config.working_directory.clone(),
                                user_id: None,
                                task_id: Some(call.id.clone()),
                                yolo_mode: self.config.yolo_mode,
                                checkpoint_manager: self.config.checkpoint_manager.clone(),
                            };
                            let result = self
                                .tools
                                .dispatch(call, context)
                                .await
                                .map_err(AgentError::Tool)?;

                            // Display: tool completed
                            if let Some(display) = &self.display_handler {
                                display.tool_completed(&call.name, &result);
                                display.flush();
                            }

                            // Track tool call via insights tracker
                            if let Some(tracker) = &self.insights_tracker {
                                let record = ToolCallRecord {
                                    tool_name: call.name.clone(),
                                    started_at: 0.0, // timestamp not tracked before call
                                    duration_ms: 0,   // duration not tracked
                                    success: true,
                                    error: None,
                                };
                                tracker.record_tool_call(record);
                            }

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
                            0, // no tool calls this turn
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
                                tools: None, // Review agent has no tools
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
                                    token_count: response.usage.as_ref().map(|u| u.output_tokens),
                                    finish_reason: Some("stop".to_string()),
                                    reasoning: response.reasoning.clone(),
                                },
                            )
                            .await;

                        // ========== Title Generation ==========
                        // Only generate title on first exchange (user + assistant = 2 messages)
                        if messages.len() == 2 {
                            if let Some(generator) = &self.title_generator {
                                if let Some(session_id) = &request.session_id {
                                    let generator = generator.clone();
                                    let user_msg = request.content.clone();
                                    let assistant_msg = response.content.clone();
                                    let store = self.session_store.clone();
                                    let sid = session_id.clone();
                                    tokio::spawn(async move {
                                        if let Some(title) = generator.generate(&user_msg, &assistant_msg).await {
                                            if let Ok(Some(mut session)) = store.get_session(&sid).await {
                                                session.title = Some(title);
                                                let _ = store.update_session(&session).await;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                    }

                    // ========== Insights: Record usage and show ==========
                    if let Some(tracker) = &self.insights_tracker {
                        if let Some(usage) = &response.usage {
                            let pricing_db = PricingDatabase::new();
                            let calculator = CostCalculator::new(&pricing_db);
                            let (provider, model) = self.config.model.split_once('/').unwrap_or(("unknown", "unknown"));
                            let cost = calculator
                                .calculate(provider, model, usage)
                                .unwrap_or(0.0);
                            tracker.record_usage(usage, cost);

                            // Show usage via display handler
                            if let Some(display) = &self.display_handler {
                                display.show_usage(&tracker.get_insights());
                                display.flush();
                            }
                        }
                    }

                    final_result = Ok(ConversationResponse {
                        content: response.content,
                        session_id: request.session_id,
                        usage: response.usage,
                    });
                    break;
                }
                crate::FinishReason::Length => {
                    // 尝试使用上下文压缩
                    // 使用 get_or_insert_with 模式复用已存在的压缩器，保持状态持久化
                    let compressor = self.context_compressor.get_or_insert_with(|| {
                        crate::ContextCompressor::new(
                            self.provider.clone(),
                            self.config.model.clone(),
                            self.provider.context_length(&model_id).unwrap_or(4096),
                        )
                    });

                    match compressor.compress(messages.clone(), None, None).await {
                        Ok(compressed_messages) => {
                            messages = compressed_messages;
                            continue;
                        }
                        Err(e) => {
                            final_result = Err(AgentError::Internal(format!(
                                "Context length exceeded and compression failed: {}",
                                e
                            )));
                            break;
                        }
                    }
                }
                crate::FinishReason::ContentFilter => {
                    final_result = Err(AgentError::ContentFiltered);
                    break;
                }
                crate::FinishReason::Other => {
                    final_result = Err(AgentError::UnknownFinishReason);
                    break;
                }
            }
        }

        // Save trajectory regardless of success/failure
        if let Some(saver) = &self.trajectory_saver {
            let completed = final_result.is_ok();
            let _ = saver.save(&messages, &self.config.model, completed);
        }

        final_result
    }
}
