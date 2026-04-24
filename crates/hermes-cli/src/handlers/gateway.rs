//! Gateway commands implementation
//!
//! Provides commands for starting, stopping, and checking status of the HTTP gateway.

use anyhow::Result;
use hermes_core::config::Config;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Display gateway configuration status
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
            if platform.token.is_some() {
                println!("      - token: configured");
            }
        }
    }

    Ok(())
}

/// Start the gateway server
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
    use crate::display::CliDisplay;

    // Initialize components
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

    // Gateway uses CliDisplay for HTTP webhook handling
    let display_handler: Option<Arc<dyn DisplayHandler>> = Some(Arc::new(CliDisplay::new()));
    let title_generator: Option<Arc<TitleGenerator>> = Some(Arc::new(TitleGenerator::with_default_model(provider.clone())));
    let trajectory_saver: Option<TrajectorySaver> = Some(TrajectorySaver::default());

    let agent = Arc::new(Agent::new(
        provider,
        tool_registry,
        session_store,
        hermes_core::AgentConfig::default(),
        NudgeConfig::default(),
        display_handler,
        title_generator,
        trajectory_saver,
    ));

    let gateway = Arc::new(Gateway::new(agent));
    let router = gateway.router();

    let addr = format!("{}:{}", config.gateway.host, port);
    let listener = TcpListener::bind(&addr).await?;
    println!("Gateway listening on {}", addr);

    axum::serve(listener, router).await?;
    Ok(())
}

/// Print setup instructions for the gateway
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
    println!("2. Set the OpenAI API key:");
    println!("   export HERMES_OPENAI_API_KEY=\"your-api-key\"");
    println!();
    println!("3. Start the gateway:");
    println!("   hermes gateway start");
    println!();
    Ok(())
}

/// Print stop instructions for the gateway
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
