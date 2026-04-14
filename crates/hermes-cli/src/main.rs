use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
use commands::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        commands::Commands::Chat {
            model,
            session,
            no_tools: _,
            credentials,
        } => {
            eprintln!("hermes chat: model={}, session={:?}, credentials={:?}",
                model, session, credentials);
            eprintln!("(Agent wiring with RetryingProvider + CredentialPool is ready when the full agent is integrated)");
            Ok(())
        }
        commands::Commands::Model { command } => {
            match command {
                commands::ModelCommands::List => {
                    eprintln!("Available models: openai/gpt-4o, openai/gpt-4-turbo, openai/gpt-3.5-turbo");
                }
                commands::ModelCommands::Set { model } => {
                    eprintln!("Setting default model to: {}", model);
                }
                commands::ModelCommands::Info { model } => {
                    eprintln!("Model info for: {}", model);
                }
            }
            Ok(())
        }
        commands::Commands::Session { command: _ } => {
            eprintln!("Session management: not yet implemented");
            Ok(())
        }
        commands::Commands::Config { command: _ } => {
            eprintln!("Config management: not yet implemented");
            Ok(())
        }
        commands::Commands::Tools { command: _ } => {
            eprintln!("Tools management: not yet implemented");
            Ok(())
        }
        commands::Commands::Skills { command: _ } => {
            eprintln!("Skills management: not yet implemented");
            Ok(())
        }
        commands::Commands::Gateway { command: _ } => {
            eprintln!("Gateway management: not yet implemented");
            Ok(())
        }
    }
}
