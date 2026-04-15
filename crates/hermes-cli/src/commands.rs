//! CLI 命令定义模块
//!
//! 使用 `clap` 库定义 Hermes Agent CLI 的所有命令和参数结构。
//!
//! ## 主要类型
//! - `Cli`: 主入口结构，包含 `command` 字段指向具体子命令
//! - `Commands`: 所有子命令的枚举，包含 7 个主要命令
//! - `ModelCommands`: 模型管理的子命令（List/Set/Info）
//! - `SessionCommands`: 会话管理的子命令（List/Show/Search/Delete）
//! - `ConfigCommands`: 配置管理的子命令（Get/Set/Show/Edit）
//! - `ToolsCommands`: 工具管理的子命令（List/Enable/Disable）
//! - `SkillsCommands`: 技能管理的子命令（List/Install/Uninstall/Search）
//! - `GatewayCommands`: 网关管理的子命令（Start/Stop/Status/Setup）
//!
//! ## 与其他模块的关系
//! - `main.rs` 使用 `clap::Parser` 解析 `Cli` 结构
//! - 各子命令的参数在此定义，解析后的值传递给对应的处理逻辑

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
