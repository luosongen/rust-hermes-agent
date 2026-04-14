use clap::Parser;
use hermes_cli::{chat, Cli, Commands, GatewayCommands};
use hermes_core::config::{Config, config_file};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Chat { model, session, no_tools, credentials } => {
            chat::run_chat(model, session, no_tools, credentials).await?;
        }
        Commands::Model { command } => {
            println!("Model command: {:?}", command);
        }
        Commands::Session { command } => {
            println!("Session command: {:?}", command);
        }
        Commands::Config { command } => {
            match command {
                hermes_cli::ConfigCommands::Get { key } => {
                    let config = Config::load().map_err(|e| anyhow::anyhow!("{}", e))?;
                    match config.get(&key) {
                        Some(value) => println!("{}", value),
                        None => anyhow::bail!("Config key not found: {}", key),
                    }
                }
                hermes_cli::ConfigCommands::Set { key, value } => {
                    let mut config = Config::load().map_err(|e| anyhow::anyhow!("{}", e))?;
                    if !config.set(&key, value) {
                        anyhow::bail!("Config key not found or cannot be set: {}", key);
                    }
                    config.save().map_err(|e| anyhow::anyhow!("{}", e))?;
                    println!("Config updated: {}", key);
                }
                hermes_cli::ConfigCommands::Show => {
                    let config = Config::load().map_err(|e| anyhow::anyhow!("{}", e))?;
                    println!("{}", config.display());
                }
                hermes_cli::ConfigCommands::Edit => {
                    let editor = Config::editor();
                    let path = config_file();
                    std::process::Command::new(&editor)
                        .arg(&path)
                        .status()
                        .map_err(|e| anyhow::anyhow!("Failed to open editor {}: {}", editor, e))?;
                }
            }
        }
        Commands::Tools { command } => {
            println!("Tools command: {:?}", command);
        }
        Commands::Skills { command } => {
            println!("Skills command: {:?}", command);
        }
        Commands::Gateway {
            command: GatewayCommands::Start { port },
        } => {
            start_gateway(port).await?;
        }
        Commands::Gateway {
            command: GatewayCommands::Stop,
        } => {
            println!("Stop gateway - TODO");
        }
        Commands::Gateway {
            command: GatewayCommands::Status,
        } => {
            println!("Gateway status - TODO");
        }
        Commands::Gateway {
            command: GatewayCommands::Setup,
        } => {
            println!("Gateway setup - TODO");
        }
    }

    Ok(())
}

async fn start_gateway(_port: u16) -> anyhow::Result<()> {
    // NOTE: Full gateway startup requires provider + credentials setup.
    // This will be completed once the provider infrastructure is ready.
    // For now, the gateway can be started programmatically:
    //
    // ```rust,ignore
    // use hermes_core::Agent;
    // use hermes_gateway::{Gateway, TelegramAdapter};
    // use hermes_memory::SqliteStore;
    // use std::sync::Arc;
    //
    // let store = SqliteStore::new("hermes.db")?;
    // let agent = Arc::new(Agent::new(...));
    // let gateway = Arc::new(Gateway::new(agent));
    // let adapter = Arc::new(TelegramAdapter::new(bot_token, verify_token));
    // gateway.register_adapter(adapter);
    // let app = gateway.router();
    // axum::Server::bind(&addr).serve(app).await?;
    // ```

    anyhow::bail!(
        "Gateway start requires provider setup. Set TELEGRAM_BOT_TOKEN, \
         TELEGRAM_VERIFY_TOKEN env vars and ensure credentials are configured. \
         Gateway infrastructure (provider manager) is still being finalized."
    );
}
