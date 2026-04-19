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

use clap::{Parser, Subcommand};

/// CLI 主入口结构
///
/// 包含一个 `command` 字段，指向用户所选的具体子命令。
/// 由 `clap` 自动根据命令行参数解析为对应的 `Commands` 枚举值。
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

/// CLI 所有子命令的枚举
///
/// 包含 7 个主要命令：
/// - `Chat`: 启动交互式聊天 REPL
/// - `Model`: 管理 AI 模型
/// - `Session`: 管理会话
/// - `Config`: 管理配置文件
/// - `Tools`: 管理工具
/// - `Skills`: 管理技能
/// - `Gateway`: 管理网关服务
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 启动交互式聊天 REPL
    Chat {
        /// 使用的模型（格式: provider/model，如 openai/gpt-4o）
        #[arg(short, long, default_value = "openai/gpt-4o")]
        model: String,

        /// 继续指定 ID 的会话
        #[arg(short, long)]
        session: Option<String>,

        /// 禁用工具执行
        #[arg(long)]
        no_tools: bool,

        /// 凭据字符串，格式为 "provider:key,provider2:key2"（启用 RetryingProvider）
        #[arg(long)]
        credentials: Option<String>,
    },

    /// 管理 AI 模型（列出可用模型、设置默认模型、查看模型信息）
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },

    /// 管理会话（列出会话、查看详情、搜索、删除）
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// 管理配置文件（读取、写入、显示、编辑）
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// 管理工具（列出工具、启用、禁用）
    Tools {
        #[command(subcommand)]
        command: ToolsCommands,
    },

    /// 管理技能（列出技能、安装、卸载、搜索）
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },

    /// 管理网关服务（启动、停止、查看状态、配置）
    Gateway {
        #[command(subcommand)]
        command: GatewayCommands,
    },
}

/// 模型管理子命令
///
/// 支持列出可用模型、设置默认模型、查看指定模型的详细信息。
#[derive(Subcommand, Debug)]
pub enum ModelCommands {
    /// 列出所有可用的模型
    List,
    /// 设置默认模型
    Set { #[arg(short, long)] model: String },
    /// 查看指定模型的详细信息
    Info { #[arg(short, long)] model: String },
}

/// 会话管理子命令
///
/// 支持列出所有会话、查看会话详情、在会话中搜索、删除会话。
#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// 列出所有会话
    List,
    /// 显示会话详情
    Show { #[arg(short, long)] id: String },
    /// 在会话中搜索
    Search { #[arg(short, long)] query: String },
    /// 删除指定会话
    Delete { #[arg(short, long)] id: String },
}

/// 配置管理子命令
///
/// 支持读取配置项、写入配置项、显示完整配置（在编辑器中编辑配置。
#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// 读取配置值
    Get { #[arg(short, long)] key: String },
    /// 设置配置值
    Set { #[arg(short, long)] key: String, #[arg(short, long)] value: String },
    /// 显示完整配置（敏感信息会脱敏）
    Show,
    /// 在 $EDITOR 中编辑配置文件
    Edit,
}

/// 工具管理子命令
///
/// 支持列出所有工具、启用指定工具、禁用指定工具。
#[derive(Subcommand, Debug)]
pub enum ToolsCommands {
    /// 列出所有可用工具
    List,
    /// 启用指定工具
    Enable { #[arg(short, long)] tool: String },
    /// 禁用指定工具
    Disable { #[arg(short, long)] tool: String },
}

/// 技能管理子命令
///
/// 支持列出所有技能、安装新技能、卸载技能、搜索技能市场。
#[derive(Subcommand, Debug)]
pub enum SkillsCommands {
    /// 列出所有已安装的技能
    List,
    /// 安装指定技能
    Install { #[arg(short, long)] skill: String },
    /// 卸载指定技能
    Uninstall { #[arg(short, long)] skill: String },
    /// 搜索技能市场
    Search { #[arg(short, long)] query: String },
}

/// 网关服务管理子命令
///
/// 支持启动网关服务器、停止网关、查看网关状态、初始化网关配置。
#[derive(Subcommand, Debug)]
pub enum GatewayCommands {
    /// 启动网关服务器
    Start {
        /// 服务器监听端口
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// 停止网关服务
    Stop,
    /// 查看网关运行状态
    Status,
    /// 初始化网关配置
    Setup,
}
