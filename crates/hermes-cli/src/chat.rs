//! Hermes Agent 交互式聊天 REPL
//!
//! 使用 tokio 的异步 I/O 实现交互式读取-执行-打印循环

use anyhow::Result;
use hermes_core::{
    Agent, AgentConfig, ConversationRequest, LlmProvider, RetryingProvider,
};
use hermes_memory::{NewSession, SessionStore, SqliteSessionStore};
use hermes_provider::OpenAiProvider;
use hermes_tool_registry::ToolRegistry;
use hermes_tools_builtin::register_builtin_tools;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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

    // 创建工具注册表
    let tool_registry = Arc::new(ToolRegistry::new());

    // 如果未禁用工具，注册内置工具
    if !no_tools {
        register_builtin_tools(&tool_registry);
    }

    // 构建 LLM Provider
    let provider: Arc<dyn LlmProvider> = if let Some(creds) = credentials {
        // 使用凭据字符串创建凭据池
        let pool = hermes_core::CredentialPool::new();
        for cred in creds.split(',') {
            let parts: Vec<&str> = cred.split(':').collect();
            if parts.len() == 2 {
                pool.add(parts[0], parts[1].to_string());
            }
        }
        // 使用 RetryingProvider 包装，添加自动重试逻辑
        Arc::new(RetryingProvider::new(
            Arc::new(OpenAiProvider::new(
                std::env::var("OPENAI_API_KEY")
                    .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))?,
                None,
            )),
            Arc::new(pool),
            hermes_core::RetryPolicy::default(),
        ))
    } else {
        // 使用默认 OpenAI Provider，从环境变量读取 API key
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))?;
        Arc::new(OpenAiProvider::new(&api_key, None))
    };

    // 构建 Agent
    let agent_config = AgentConfig {
        model: model.clone(),
        ..Default::default()
    };
    let agent = Arc::new(Agent::new(
        provider,
        tool_registry,
        session_store.clone(),
        agent_config,
    ));

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

    // REPL 循环：读取用户输入，发送到 Agent，显示响应
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let agent = Arc::clone(&agent);
    let session_id = Arc::new(session_id);

    loop {
        print!("> ");
        tokio::io::stdout().flush().await?;

        match reader.next_line().await {
            Ok(Some(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // 克隆会话 ID 用于此次请求
                let sid = (*session_id).clone();
                // 调用 Agent 处理对话
                let response = agent
                    .run_conversation(ConversationRequest {
                        content: line.to_string(),
                        session_id: Some(sid),
                        system_prompt: None,
                    })
                    .await;

                match response {
                    Ok(resp) => {
                        println!("[Agent] {}\n", resp.content);
                    }
                    Err(e) => {
                        eprintln!("[错误] {}\n", e);
                    }
                }
            }
            Ok(None) => {
                // EOF (Ctrl+D)
                println!("\n再见!");
                break;
            }
            Err(e) => {
                eprintln!("读取输入错误: {}", e);
                break;
            }
        }
    }

    Ok(())
}
