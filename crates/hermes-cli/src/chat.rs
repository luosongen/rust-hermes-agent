//! Hermes Agent 交互式聊天 REPL
//!
//! 使用 tokio 的异步 I/O 实现交互式读取-执行-打印循环（Read-Eval-Print Loop）。
//!
//! ## 模块用途
//! 实现 CLI 的 `chat` 子命令：初始化 Agent、创建/恢复会话、循环读取用户输入、调用 Agent 处理并打印响应。
//!
//! ## 主要函数
//! - `run_chat(...)`: 异步函数，启动完整的聊天会话流程
//!
//! ## 核心流程
//! 1. 初始化 SQLite 会话存储（`SqliteSessionStore`）
//! 2. 创建工具注册表（`ToolRegistry`），可选注册内置工具
//! 3. 根据凭据构建 LLM Provider（`OpenAiProvider` 或带重试的 `RetryingProvider`）
//! 4. 创建 Agent 实例
//! 5. 创建或恢复会话
//! 6. 进入 REPL 循环：读取 stdin → 调用 `agent.run_conversation()` → 打印响应
//!
//! ## 依赖关系
//! - `hermes_core`: `Agent`、`AgentConfig`、`ConversationRequest`、`RetryingProvider`、`CredentialPool`、`RetryPolicy`
//! - `hermes_memory`: `NewSession`、`SessionStore`、`SqliteSessionStore`
//! - `hermes_provider`: `OpenAiProvider`
//! - `hermes_tool_registry`: `ToolRegistry`
//! - `hermes_tools_builtin`: `register_builtin_tools`

use anyhow::Result;
use hermes_core::{
    Agent, AgentConfig, ConversationRequest, DisplayHandler, InMemoryInsightsTracker,
    InsightsTracker, LlmProvider, PoolStrategy, RateLimitTracker, RetryConfig, TitleGenerator,
    TrajectorySaver,
};
use crate::display::CliDisplay;
use crate::ui::{
    LineReader,
    StreamingOutput,
};
use hermes_environment::{EnvironmentManager, LocalEnvironment};
use hermes_memory::{NewSession, SessionStore, SqliteSessionStore};
use hermes_provider::{OpenAiProvider, AnthropicProvider, OpenRouterProvider, GlmProvider, MiniMaxProvider, KimiProvider, DeepSeekProvider, QwenProvider};
use hermes_tool_registry::ToolRegistry;
use hermes_tools_builtin::{register_builtin_tools, register_skill_tools, load_skill_registry_and_manager};
use std::sync::Arc;

/// 根据 model ID 创建对应的 provider
/// model ID 格式: "provider/model-name" (e.g., "anthropic/claude-3-5-sonnet")
fn create_provider_for_model(model: &str, api_key: Option<&str>) -> Result<Arc<dyn LlmProvider>> {
    let (provider_name, _) = model.split_once('/').unwrap_or((model, ""));

    match provider_name {
        "openai" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .or_else(|| std::env::var("HERMES_OPENAI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("OpenAI API key not found"))?;
            Ok(Arc::new(OpenAiProvider::new(key, None)))
        }
        "anthropic" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Anthropic API key not found"))?;
            Ok(Arc::new(AnthropicProvider::new(key)))
        }
        "openrouter" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("OpenRouter API key not found"))?;
            Ok(Arc::new(OpenRouterProvider::new(key)))
        }
        "glm" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("GLM_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("GLM API key not found"))?;
            Ok(Arc::new(GlmProvider::new(key)))
        }
        "minimax" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("MiniMax API key not found"))?;
            Ok(Arc::new(MiniMaxProvider::new(key)))
        }
        "kimi" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("KIMI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Kimi API key not found"))?;
            Ok(Arc::new(KimiProvider::new(key)))
        }
        "deepseek" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("DeepSeek API key not found"))?;
            Ok(Arc::new(DeepSeekProvider::new(key)))
        }
        "qwen" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("QWEN_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Qwen API key not found"))?;
            Ok(Arc::new(QwenProvider::new(key)))
        }
        _ => Err(anyhow::anyhow!("Unsupported provider: {}", provider_name)),
    }
}

/// 运行交互式聊天会话
///
/// # 参数
/// - `model`: 使用的模型（如 "openai/gpt-4o"）
/// - `session_id`: 可选的会话 ID，用于继续之前的对话
/// - `no_tools`: 是否禁用工具执行
/// - `credentials`: 可选的凭据字符串，格式为 "provider:key,provider2:key2"
///
/// # 示例
/// ```ignore
/// run_chat("openai/gpt-4o".to_string(), None, false, None).await?;
/// ```
pub async fn run_chat(
    model: String,
    session_id: Option<String>,
    no_tools: bool,
    credentials: Option<String>,
) -> Result<()> {
    // 初始化组件
    // 创建 SQLite 会话存储，使用 Arc 共享
    let session_store = Arc::new(SqliteSessionStore::new("hermes.db".into()).await?);

    // 创建执行环境（根据配置选择本地、Docker 或 SSH）
    let environment = if let Ok(config) = hermes_core::config::Config::load() {
        let env_config = hermes_environment::EnvironmentConfig {
            env_type: config.environment.env_type.parse().unwrap_or(hermes_environment::EnvironmentType::Local),
            working_directory: config.environment.working_directory.clone(),
            docker: hermes_environment::DockerConfigSerde {
                container: config.environment.docker.container.clone(),
                docker_host: config.environment.docker.docker_host.clone(),
                working_directory: config.environment.docker.working_directory.clone(),
                auto_start: config.environment.docker.auto_start,
                user: config.environment.docker.user.clone(),
            },
            ssh: hermes_environment::SSHConfigSerde {
                host: config.environment.ssh.host.clone(),
                port: config.environment.ssh.port,
                user: config.environment.ssh.user.clone(),
                private_key: config.environment.ssh.private_key.clone(),
                password: config.environment.ssh.password.clone(),
                working_directory: config.environment.ssh.working_directory.clone(),
                ssh_options: config.environment.ssh.ssh_options.clone(),
            },
            env_vars: std::collections::HashMap::new(),
        };
        EnvironmentManager::from_config(&env_config)
            .unwrap_or_else(|_| Arc::new(LocalEnvironment::new(".")))
    } else {
        Arc::new(LocalEnvironment::new("."))
    };

    // 创建工具注册表
    let tool_registry = Arc::new(ToolRegistry::new());

    // 如果未禁用工具，注册内置工具（注入 Environment）
    if !no_tools {
        register_builtin_tools(&tool_registry, environment.clone());
        // 加载技能注册表和管理器，并注册技能管理工具
        let (_, skill_manager, skill_executor) = load_skill_registry_and_manager();
        register_skill_tools(&tool_registry, skill_manager, skill_executor);
    }

    // 构建 LLM Provider
    let provider: Arc<dyn LlmProvider> = if let Some(creds) = credentials {
        // 使用凭据字符串创建凭据池
        let pool = hermes_core::CredentialPool::new(PoolStrategy::RoundRobin);
        for cred in creds.split(',') {
            let parts: Vec<&str> = cred.split(':').collect();
            if parts.len() == 2 {
                pool.add(parts[0], parts[1], parts[1]);
            }
        }
        // 使用 RetryingProvider 包装
        let model_key = model.clone();
        let inner_provider = create_provider_for_model(&model_key, None)?;
        Arc::new(hermes_core::RetryingProvider::new(
            inner_provider,
            Arc::new(pool),
            hermes_core::RetryPolicy::default(),
        ))
    } else {
        // 根据 model ID 创建对应的 provider
        create_provider_for_model(&model, credentials.as_deref())?
    };

    // 构建 Agent
    let agent_config = AgentConfig {
        model: model.clone(),
        ..Default::default()
    };
    let nudge_config = hermes_core::config::Config::load().map(|c| c.nudge).unwrap_or_default();

    // 创建显示处理器
    let display_handler: Option<Arc<dyn DisplayHandler>> = Some(Arc::new(CliDisplay::new()));

    // 创建标题生成器（复用同一个 provider）
    let title_generator = Some(Arc::new(TitleGenerator::with_default_model(provider.clone())));

    // 创建轨迹保存器
    let trajectory_saver = Some(TrajectorySaver::default());

    // 创建追踪器
    let insights_tracker: Option<Arc<dyn InsightsTracker>> =
        Some(Arc::new(InMemoryInsightsTracker::new("session", "openai", &model)));
    let rate_limit_tracker: Option<Arc<RateLimitTracker>> =
        Some(Arc::new(RateLimitTracker::new()));

    let retry_config = RetryConfig::default();

    let agent: Arc<tokio::sync::RwLock<Agent>> = Arc::new(tokio::sync::RwLock::new(Agent::new(
        provider,
        tool_registry,
        session_store.clone(),
        agent_config,
        nudge_config,
        display_handler,
        title_generator,
        trajectory_saver,
        insights_tracker,
        rate_limit_tracker,
        retry_config,
    )));

    // 确定会话 ID
    // 如果提供了会话 ID，则使用它；否则创建新会话
    let session_id = if let Some(ref sid) = session_id {
        sid.clone()
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        // 在存储中创建新会话
        session_store
            .create_session(NewSession {
                id: new_id.clone(),
                source: "cli".to_string(),
                user_id: None,
                model: Some(model.clone()),
            })
            .await?;
        new_id
    };

    println!("[Session: {}] ({})", session_id, model);
    println!("输入消息后按回车发送。Ctrl+C 退出。\n");

    // 创建 UI 组件
    let loading_animation = StreamingOutput::new();
    let line_reader = LineReader::new(Some("hermes_history.txt"));
    let agent = Arc::clone(&agent);
    let session_id = Arc::new(session_id);

    loop {
        let line = match line_reader.read_line("> ").await {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // 克隆会话 ID 用于此次请求
        let sid = (*session_id).clone();

        // 显示加载动画
        loading_animation.start_loading("处理中");

        // 调用 Agent 处理对话
        let response = agent
            .write().await
            .run_conversation(ConversationRequest {
                content: line.to_string(),
                session_id: Some(sid),
                system_prompt: None,
            })
            .await;

        // 停止加载动画
        loading_animation.stop_loading();

        match response {
            Ok(resp) => {
                println!("[Agent] {}\n", resp.content);
            }
            Err(e) => {
                eprintln!("[错误] {}\n", e);
            }
        }
    }

    Ok(())
}
