# Gateway 完整启动示例

本示例展示如何启动 Hermes Gateway 并注册所有平台适配器。

## 示例代码

```rust
use std::sync::Arc;

use hermes_core::Agent;
use hermes_core::config::Config;
use hermes_core::nudge::NudgeConfig;
use hermes_core::{LlmProvider, ToolDispatcher, DisplayHandler, TitleGenerator, TrajectorySaver, RetryConfig};
use hermes_gateway::Gateway;
use hermes_memory::SqliteSessionStore;
use hermes_provider::OpenAiProvider;
use hermes_tool_registry::ToolRegistry;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting Hermes Gateway...");

    // 加载配置
    let config = Config::load()?;
    tracing::info!("Configuration loaded");

    // 初始化存储
    let db_path = PathBuf::from("./hermes.db");
    let session_store: Arc<dyn hermes_core::SessionStore> =
        Arc::new(SqliteSessionStore::new(db_path).await?);
    tracing::info!("Session store initialized");

    // 初始化工具注册表
    let tool_registry = Arc::new(ToolRegistry::new()) as Arc<dyn ToolDispatcher>;
    tracing::info!("Tool registry initialized");

    // 初始化 LLM Provider
    let api_key = config
        .credentials
        .get("openai")
        .cloned()
        .ok_or_else(|| {
            Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "OpenAI API key not configured. Set HERMES_OPENAI_API_KEY or configure in config.",
            )) as Box<dyn std::error::Error>
        })?;
    let provider: Arc<dyn LlmProvider> = Arc::new(OpenAiProvider::new(api_key, None));
    tracing::info!("LLM Provider initialized");

    // 创建 Agent
    let display_handler: Option<Arc<dyn DisplayHandler>> = None;
    let title_generator: Option<Arc<TitleGenerator>> = None;
    let trajectory_saver: Option<TrajectorySaver> = None;
    let retry_config = RetryConfig::default();

    let agent = Arc::new(Agent::new(
        provider,
        tool_registry,
        session_store,
        hermes_core::AgentConfig::default(),
        NudgeConfig::default(),
        display_handler,
        title_generator,
        trajectory_saver,
        None,
        None,
        retry_config,
    ));
    tracing::info!("Agent created");

    // 创建 Gateway
    let gateway = Arc::new(Gateway::new(agent));

    // 注册平台适配器
    register_adapters(&gateway, &config).await;
    tracing::info!("Platform adapters registered");

    // 构建路由
    let router = gateway.router();
    tracing::info!("Router built with all webhook endpoints");

    // 启动服务器
    let addr = format!("{}:{}", config.gateway.host, config.gateway.port);
    tracing::info!("Gateway listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

/// 注册所有平台适配器
async fn register_adapters(gateway: &Arc<Gateway>, config: &Config) {
    use hermes_core::gateway::PlatformAdapter;
    use hermes_platform_telegram::TelegramAdapter;
    use hermes_platform_wecom::WeComAdapter;
    use hermes_platform_dingtalk::DingTalkAdapter;
    use hermes_platform_feishu::FeishuAdapter;
    use hermes_platform_sms::SmsAdapter;

    // Telegram
    if let Some(platform) = config.gateway.platforms.get("telegram") {
        if let Some(bot_token) = &platform.bot_token {
            let verify_token = platform.verify_token.clone().unwrap_or_default();
            let adapter = Arc::new(TelegramAdapter::new(
                bot_token.clone(),
                verify_token,
            ));
            gateway.register_adapter(adapter);
            tracing::info!("Registered Telegram adapter");
        }
    }

    // WeCom
    if let Some(platform) = config.gateway.platforms.get("wecom") {
        if let (Some(corp_id), Some(agent_id), Some(token), Some(aes_key)) = (
            &platform.corp_id,
            &platform.agent_id,
            &platform.token,
            &platform.aes_key,
        ) {
            let adapter = Arc::new(WeComAdapter::new(
                corp_id.clone(),
                agent_id.clone(),
                token.clone(),
                aes_key.clone(),
            ));
            gateway.register_adapter(adapter);
            tracing::info!("Registered WeCom adapter");
        }
    }

    // DingTalk
    if let Some(platform) = config.gateway.platforms.get("dingtalk") {
        if let (Some(app_key), Some(app_secret)) =
            (&platform.app_key, &platform.app_secret)
        {
            let adapter = Arc::new(
                DingTalkAdapter::new()
                    .with_credentials(app_key.clone(), app_secret.clone()),
            );
            gateway.register_adapter(adapter);
            tracing::info!("Registered DingTalk adapter");
        }
    }

    // Feishu
    if let Some(platform) = config.gateway.platforms.get("feishu") {
        if let (Some(app_id), Some(app_secret)) =
            (&platform.feishu_app_id, &platform.feishu_app_secret)
        {
            let adapter = Arc::new(
                FeishuAdapter::new()
                    .with_credentials(app_id.clone(), app_secret.clone()),
            );
            gateway.register_adapter(adapter);
            tracing::info!("Registered Feishu adapter");
        }
    }

    // SMS (Twilio)
    if let Some(platform) = config.gateway.platforms.get("sms") {
        if let (Some(account_sid), Some(auth_token)) = (
            &platform.twilio_account_sid,
            &platform.twilio_auth_token,
        ) {
            let adapter = Arc::new(
                SmsAdapter::new()
                    .with_credentials(account_sid.clone(), auth_token.clone())
                    .with_from(platform.twilio_from_number.clone().unwrap_or_default()),
            );
            gateway.register_adapter(adapter);
            tracing::info!("Registered SMS (Twilio) adapter");
        }
    }
}
```

## 运行方式

### 1. 设置环境变量

```bash
# OpenAI API Key (必需)
export HERMES_OPENAI_API_KEY="sk-..."

# Telegram (可选)
export HERMES_TELEGRAM_BOT_TOKEN="your-bot-token"
export HERMES_TELEGRAM_VERIFY_TOKEN="your-verify-token"

# DingTalk (可选)
export HERMES_DINGTALK_APP_KEY="your-app-key"
export HERMES_DINGTALK_APP_SECRET="your-app-secret"

# Feishu (可选)
export HERMES_FEISHU_APP_ID="your-app-id"
export HERMES_FEISHU_APP_SECRET="your-app-secret"

# SMS/Twilio (可选)
export HERMES_TWILIO_ACCOUNT_SID="your-account-sid"
export HERMES_TWILIO_AUTH_TOKEN="your-auth-token"
export HERMES_TWILIO_FROM_NUMBER="+1234567890"
```

### 2. 配置文件方式

创建 `~/.config/hermes-agent/config.toml`:

```toml
[gateway]
port = 8080
host = "0.0.0.0"

[[gateway.platforms.telegram]]
bot_token = "your-bot-token"
verify_token = "your-verify-token"

[[gateway.platforms.dingtalk]]
app_key = "your-app-key"
app_secret = "your-app-secret"

[[gateway.platforms.feishu]]
feishu_app_id = "your-app-id"
feishu_app_secret = "your-app-secret"

[[gateway.platforms.sms]]
twilio_account_sid = "your-account-sid"
twilio_auth_token = "your-auth-token"
twilio_from_number = "+1234567890"
```

### 3. 启动 Gateway

```bash
# 使用 hermes CLI
hermes gateway start

# 或直接运行 (需要先设置 API key)
cargo run -p hermes-cli -- gateway start
```

## Webhook 端点

启动后可用以下端点：

| 端点 | 平台 |
|------|------|
| `POST /webhook/telegram` | Telegram |
| `POST /webhook/wecom` | 企业微信 |
| `POST /webhook/dingtalk` | 钉钉 |
| `POST /webhook/feishu` | 飞书 |
| `POST /webhook/weixin` | 微信 |
| `POST /webhook/sms` | Twilio SMS |
| `GET /health` | 健康检查 |
