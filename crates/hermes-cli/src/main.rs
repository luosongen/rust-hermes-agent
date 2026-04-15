//! Hermes Agent CLI 主程序入口
//!
//! 负责解析命令行参数并分发到对应的子命令模块。
//!
//! ## 架构
//! - 使用 `clap` 进行 CLI 参数解析
//! - 使用 `tracing_subscriber` 初始化日志
//! - 使用 `tokio` 异步运行时执行命令
//!
//! ## 主要子命令
//! - `chat`: 启动交互式聊天 REPL
//! - `model`: 管理 AI 模型（列表、设置、查看信息）
//! - `session`: 管理会话（列表、查看、搜索、删除）
//! - `config`: 管理配置文件
//! - `tools`: 管理工具
//! - `skills`: 管理技能
//! - `gateway`: 管理网关服务
//!
//! ## 模块关系
//! - `commands.rs`: 定义所有 CLI 子命令结构
//! - `chat.rs`: 交互式聊天的 REPL 实现

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
