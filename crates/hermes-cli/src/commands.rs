use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "hermes",
    about = "Hermes Agent - AI Assistant",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start an interactive chat
    Chat {
        /// Model to use (provider/model)
        #[arg(short, long, default_value = "openai/gpt-4o")]
        model: String,

        /// Session ID to continue
        #[arg(short, long)]
        session: Option<String>,

        /// Disable tools
        #[arg(long)]
        no_tools: bool,

        /// Credentials in format provider:key,provider2:key2 (enables RetryingProvider)
        #[arg(long)]
        credentials: Option<String>,
    },

    /// Manage models
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },

    /// Manage sessions
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Manage tools
    Tools {
        #[command(subcommand)]
        command: ToolsCommands,
    },

    /// Manage skills
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },

    /// Manage gateway
    Gateway {
        #[command(subcommand)]
        command: GatewayCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum ModelCommands {
    /// List available models
    List,
    /// Set default model
    Set { #[arg(short, long)] model: String },
    /// Show model info
    Info { #[arg(short, long)] model: String },
}

#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List sessions
    List,
    /// Show session details
    Show { #[arg(short, long)] id: String },
    /// Search sessions
    Search { #[arg(short, long)] query: String },
    /// Delete session
    Delete { #[arg(short, long)] id: String },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Get config value
    Get { #[arg(short, long)] key: String },
    /// Set config value
    Set { #[arg(short, long)] key: String, #[arg(short, long)] value: String },
    /// Show full config (redacts secrets)
    Show,
    /// Edit config file in $EDITOR
    Edit,
}

#[derive(Subcommand, Debug)]
pub enum ToolsCommands {
    /// List tools
    List,
    /// Enable tool
    Enable { #[arg(short, long)] tool: String },
    /// Disable tool
    Disable { #[arg(short, long)] tool: String },
}

#[derive(Subcommand, Debug)]
pub enum SkillsCommands {
    /// List skills
    List,
    /// Install skill
    Install { #[arg(short, long)] skill: String },
    /// Uninstall skill
    Uninstall { #[arg(short, long)] skill: String },
    /// Search skills
    Search { #[arg(short, long)] query: String },
}

#[derive(Subcommand, Debug)]
pub enum GatewayCommands {
    /// Start gateway server
    Start {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Stop gateway
    Stop,
    /// Gateway status
    Status,
    /// Setup gateway
    Setup,
}
