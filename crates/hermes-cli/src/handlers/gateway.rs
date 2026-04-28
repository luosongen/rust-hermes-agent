//! 网关命令处理器
//!
//! 提供 HTTP 网关的启动、停止和状态检查命令。

use anyhow::Result;
use hermes_core::config::Config;
use std::sync::Arc;
use tokio::net::TcpListener;

/// 显示网关配置状态
pub async fn gateway_status() -> Result<()> {
    let config = Config::load()?;
    let gateway = &config.gateway;

    println!("Gateway Configuration:");
    println!("  Host: {}", gateway.host);
    println!("  Port: {}", gateway.port);

    if gateway.platforms.is_empty() {
        println!("  Platforms: (none configured)");
    } else {
        println!("  Platforms:");
        for (name, platform) in &gateway.platforms {
            println!("    {}", name);
            if platform.bot_token.is_some() {
                println!("      - bot_token: configured");
            }
            if platform.verify_token.is_some() {
                println!("      - verify_token: configured");
            }
            if platform.corp_id.is_some() {
                println!("      - corp_id: configured");
            }
            if platform.agent_id.is_some() {
                println!("      - agent_id: configured");
            }
            if platform.token.is_some() {
                println!("      - token: configured");
            }
            if platform.aes_key.is_some() {
                println!("      - aes_key: configured");
            }
            if platform.app_key.is_some() {
                println!("      - app_key: configured");
            }
            if platform.app_secret.is_some() {
                println!("      - app_secret: configured");
            }
            if platform.feishu_app_id.is_some() {
                println!("      - feishu_app_id: configured");
            }
            if platform.feishu_app_secret.is_some() {
                println!("      - feishu_app_secret: configured");
            }
            if platform.verification_token.is_some() {
                println!("      - verification_token: configured");
            }
            if platform.encrypt_key.is_some() {
                println!("      - encrypt_key: configured");
            }
            if platform.wx_app_id.is_some() {
                println!("      - wx_app_id: configured");
            }
            if platform.wx_app_secret.is_some() {
                println!("      - wx_app_secret: configured");
            }
            if platform.twilio_account_sid.is_some() {
                println!("      - twilio_account_sid: configured");
            }
            if platform.twilio_auth_token.is_some() {
                println!("      - twilio_auth_token: configured");
            }
            if platform.twilio_from_number.is_some() {
                println!("      - twilio_from_number: configured");
            }
        }
    }

    Ok(())
}

/// 启动网关服务器
pub async fn start_gateway(port: u16) -> Result<()> {
    use hermes_core::Agent;
    use hermes_memory::SqliteSessionStore;
    use hermes_provider::OpenAiProvider;
    use hermes_tool_registry::ToolRegistry;
    use hermes_gateway::Gateway;
    use std::path::PathBuf;
    use hermes_core::{LlmProvider, ToolDispatcher};
    use hermes_core::nudge::NudgeConfig;
    use hermes_core::DisplayHandler;
    use hermes_core::TitleGenerator;
    use hermes_core::TrajectorySaver;
    use hermes_core::RetryConfig;
    use crate::display::CliDisplay;

    // 初始化各组件
    let db_path = PathBuf::from("./hermes.db");
    let session_store: Arc<dyn hermes_core::SessionStore> = Arc::new(SqliteSessionStore::new(db_path).await?);
    let tool_registry = Arc::new(ToolRegistry::new()) as Arc<dyn ToolDispatcher>;

    let config = Config::load()?;
    let api_key = config
        .credentials
        .get("openai")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("OpenAI API key not configured. Set HERMES_OPENAI_API_KEY or configure in config."))?;

    let provider: Arc<dyn LlmProvider> = Arc::new(OpenAiProvider::new(api_key, None));

    // 网关使用 CliDisplay 处理 HTTP webhook
    let display_handler: Option<Arc<dyn DisplayHandler>> = Some(Arc::new(CliDisplay::new()));
    let title_generator: Option<Arc<TitleGenerator>> = Some(Arc::new(TitleGenerator::with_default_model(provider.clone())));
    let trajectory_saver: Option<TrajectorySaver> = Some(TrajectorySaver::default());
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

    let gateway = Arc::new(Gateway::new(agent));
    let router = gateway.router();

    let addr = format!("{}:{}", config.gateway.host, port);
    let listener = TcpListener::bind(&addr).await?;
    println!("Gateway listening on {}", addr);

    axum::serve(listener, router).await?;
    Ok(())
}

/// 打印网关设置说明
pub fn setup_gateway() -> Result<()> {
    println!("Gateway Setup Instructions:");
    println!();
    println!("1. Configure your platform tokens in ~/.config/hermes-agent/config.toml:");
    println!();
    println!("   [gateway]");
    println!("   port = 8080");
    println!("   host = \"0.0.0.0\"");
    println!();
    println!("   [[gateway.platforms.telegram]]");
    println!("   bot_token = \"your-telegram-bot-token\"");
    println!("   verify_token = \"your-verify-token\"");
    println!();
    println!("   [[gateway.platforms.wecom]]");
    println!("   corp_id = \"your-corp-id\"");
    println!("   agent_id = \"your-agent-id\"");
    println!("   token = \"your-token\"");
    println!("   aes_key = \"your-aes-key\"");
    println!();
    println!("   [[gateway.platforms.dingtalk]]");
    println!("   app_key = \"your-app-key\"");
    println!("   app_secret = \"your-app-secret\"");
    println!();
    println!("   [[gateway.platforms.feishu]]");
    println!("   feishu_app_id = \"your-app-id\"");
    println!("   feishu_app_secret = \"your-app-secret\"");
    println!("   verification_token = \"your-verification-token\"");
    println!("   encrypt_key = \"your-encrypt-key\"");
    println!();
    println!("   [[gateway.platforms.weixin]]");
    println!("   wx_app_id = \"your-app-id\"");
    println!("   wx_app_secret = \"your-app-secret\"");
    println!("   wx_token = \"your-token\"");
    println!("   wx_aes_key = \"your-aes-key\"");
    println!();
    println!("   [[gateway.platforms.sms]]");
    println!("   twilio_account_sid = \"your-account-sid\"");
    println!("   twilio_auth_token = \"your-auth-token\"");
    println!("   twilio_from_number = \"+1234567890\"");
    println!();
    println!("2. Or use environment variables:");
    println!("   export HERMES_OPENAI_API_KEY=\"your-api-key\"");
    println!("   export HERMES_DINGTALK_APP_KEY=\"your-app-key\"");
    println!("   export HERMES_FEISHU_APP_ID=\"your-app-id\"");
    println!("   export HERMES_TWILIO_ACCOUNT_SID=\"your-account-sid\"");
    println!();
    println!("3. Start the gateway:");
    println!("   hermes gateway start");
    println!();
    Ok(())
}

/// 打印网关停止说明
pub fn stop_gateway() -> Result<()> {
    println!("To stop the gateway:");
    println!();
    println!("1. Find the gateway process:");
    println!("   ps aux | grep hermes");
    println!("   lsof -i :8080");
    println!();
    println!("2. Kill the process:");
    println!("   kill <PID>");
    println!();
    println!("   Or if using pkill:");
    println!("   pkill -f hermes-gateway");
    println!();
    Ok(())
}
