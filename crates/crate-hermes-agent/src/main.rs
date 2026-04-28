use clap::Parser;
use hermes_cli::{chat, Cli, Commands, GatewayCommands};
use hermes_cli::handlers::{gateway, model, session, skills, tools};
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
        Commands::Chat { model, session, no_tools, credentials, yolo, fast } => {
            chat::run_chat(model, session, no_tools, credentials, yolo, fast).await?;
        }
        Commands::Model { command } => {
            match command {
                hermes_cli::ModelCommands::List => model::list_models()?,
                hermes_cli::ModelCommands::Set { model } => model::set_default_model(&model)?,
                hermes_cli::ModelCommands::Info { model } => model::model_info(&model)?,
            }
        }
        Commands::Session { command } => {
            match command {
                hermes_cli::SessionCommands::List => session::list_sessions().await?,
                hermes_cli::SessionCommands::Show { id } => session::show_session(&id).await?,
                hermes_cli::SessionCommands::Search { query } => session::search_sessions(&query).await?,
                hermes_cli::SessionCommands::Delete { id } => session::delete_session(&id).await?,
            }
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
            match command {
                hermes_cli::ToolsCommands::List => tools::list_tools()?,
                hermes_cli::ToolsCommands::Enable { tool } => tools::enable_tool(&tool)?,
                hermes_cli::ToolsCommands::Disable { tool } => tools::disable_tool(&tool)?,
            }
        }
        Commands::Skills { command } => {
            match command {
                hermes_cli::SkillsCommands::List => skills::list_skills()?,
                hermes_cli::SkillsCommands::Search { query } => skills::search_skills(&query)?,
                hermes_cli::SkillsCommands::Install { skill } => skills::install_skill(&skill)?,
                hermes_cli::SkillsCommands::Uninstall { skill } => skills::uninstall_skill(&skill)?,
            }
        }
        Commands::Gateway {
            command: GatewayCommands::Start { port },
        } => {
            gateway::start_gateway(port).await?;
        }
        Commands::Gateway {
            command: GatewayCommands::Stop,
        } => {
            gateway::stop_gateway()?;
        }
        Commands::Gateway {
            command: GatewayCommands::Status,
        } => {
            gateway::gateway_status().await?;
        }
        Commands::Gateway {
            command: GatewayCommands::Setup,
        } => {
            gateway::setup_gateway()?;
        }
    }

    Ok(())
}
