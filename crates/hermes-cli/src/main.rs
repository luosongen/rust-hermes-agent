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
//! - `handlers/`: 命令处理器实现

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod handlers;
mod chat;
mod display;
mod ui;
pub mod slash_commands;
pub mod background_tasks;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = commands::Cli::parse();

    match cli.command {
        commands::Commands::Chat {
            model,
            session,
            no_tools,
            credentials,
            yolo,
            fast,
        } => {
            crate::chat::run_chat(model, session, no_tools, credentials, yolo, fast).await?;
        }
        commands::Commands::Model { command } => {
            match command {
                commands::ModelCommands::List => {
                    handlers::model::list_models()?;
                }
                commands::ModelCommands::Set { model } => {
                    handlers::model::set_default_model(&model)?;
                }
                commands::ModelCommands::Info { model } => {
                    handlers::model::model_info(&model)?;
                }
            }
        }
        commands::Commands::Session { command } => {
            match command {
                commands::SessionCommands::List => {
                    handlers::session::list_sessions().await?;
                }
                commands::SessionCommands::Show { id } => {
                    handlers::session::show_session(&id).await?;
                }
                commands::SessionCommands::Search { query } => {
                    handlers::session::search_sessions(&query).await?;
                }
                commands::SessionCommands::Delete { id } => {
                    handlers::session::delete_session(&id).await?;
                }
            }
        }
        commands::Commands::Config { command } => {
            match command {
                commands::ConfigCommands::Show => {
                    handlers::config::show_config()?;
                }
                commands::ConfigCommands::Get { key } => {
                    handlers::config::get_config(&key)?;
                }
                commands::ConfigCommands::Set { key, value } => {
                    handlers::config::set_config(&key, &value)?;
                }
                commands::ConfigCommands::Edit => {
                    handlers::config::edit_config()?;
                }
            }
        }
        commands::Commands::Tools { command } => {
            match command {
                commands::ToolsCommands::List => {
                    handlers::tools::list_tools()?;
                }
                commands::ToolsCommands::Enable { tool } => {
                    handlers::tools::enable_tool(&tool)?;
                }
                commands::ToolsCommands::Disable { tool } => {
                    handlers::tools::disable_tool(&tool)?;
                }
            }
        }
        commands::Commands::Skills { command } => {
            match command {
                commands::SkillsCommands::List => {
                    handlers::skills::list_skills()?;
                }
                commands::SkillsCommands::Install { skill } => {
                    handlers::skills::install_skill(&skill)?;
                }
                commands::SkillsCommands::Uninstall { skill } => {
                    handlers::skills::uninstall_skill(&skill)?;
                }
                commands::SkillsCommands::Search { query } => {
                    handlers::skills::search_skills(&query)?;
                }
            }
        }
        commands::Commands::Gateway { command } => {
            match command {
                commands::GatewayCommands::Start { port } => {
                    handlers::gateway::start_gateway(port).await?;
                }
                commands::GatewayCommands::Stop => {
                    handlers::gateway::stop_gateway()?;
                }
                commands::GatewayCommands::Status => {
                    handlers::gateway::gateway_status().await?;
                }
                commands::GatewayCommands::Setup => {
                    handlers::gateway::setup_gateway()?;
                }
            }
        }
    }
    Ok(())
}
