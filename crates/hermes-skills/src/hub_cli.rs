//! Hub CLI 命令行接口模块
//!
//! 提供 skill hub 的命令行操作，包括浏览、搜索、安装、更新和卸载技能

use clap::{Parser, Subcommand};
use crate::hub::{HubClient, HubError};
use std::path::PathBuf;

/// Hub CLI 命令行入口
#[derive(Parser)]
pub struct HubCli {
    /// 子命令
    #[command(subcommand)]
    pub command: HubCommand,
}

/// Hub 子命令
#[derive(Subcommand)]
pub enum HubCommand {
    /// 浏览可用技能
    Browse {
        /// 指定分类浏览
        #[arg(long)]
        category: Option<String>,
    },
    /// 按名称或描述搜索技能
    Search {
        /// 搜索关键词
        query: String,
    },
    /// 从市场安装技能
    Install {
        /// 技能 ID（格式：category/name）
        skill_id: String,
        /// 跳过安全检查
        #[arg(long)]
        force: bool,
    },
    /// 从 Git URL 安装
    InstallFromGit {
        /// Git 仓库 URL
        git_url: String,
        /// 分类
        #[arg(long)]
        category: String,
        /// 技能名称
        #[arg(long)]
        name: String,
        /// 分支
        #[arg(long, default_value = "main")]
        branch: String,
    },
    /// 同步市场索引
    Sync {
        /// 强制刷新
        #[arg(long)]
        force: bool,
    },
    /// 列出已安装的技能
    List,
    /// 更新技能
    Update {
        skill_id: String,
    },
    /// 更新所有技能
    UpdateAll,
    /// 卸载技能
    Uninstall {
        skill_id: String,
    },
    /// 查看技能详情
    View {
        skill_id: String,
    },
    /// 查看安全扫描结果
    ViewSecurity {
        skill_id: String,
    },
    /// 信任技能
    Trust {
        skill_id: String,
    },
    /// 取消信任技能
    Untrust {
        skill_id: String,
    },
}

/// 执行 Hub CLI 命令
pub async fn run_hub_command(cli: HubCli) -> Result<(), HubError> {
    let home_dir = dirs::home_dir()
        .map(|h| h.join(".hermes"))
        .unwrap_or_else(|| PathBuf::from(".hermes"));

    let hub = HubClient::new(home_dir)?;

    match cli.command {
        HubCommand::Browse { category } => {
            if let Some(cat) = category {
                hub.browse.print_skill_list(&cat)?;
            } else {
                hub.browse.print_category_list()?;
            }
        }
        HubCommand::Search { query } => {
            let skills = hub.index.list_skills()?;
            for skill in skills {
                if skill.name.contains(&query) || skill.description.contains(&query) {
                    println!("{}: {}", skill.id, skill.description);
                }
            }
        }
        HubCommand::Install { skill_id, force } => {
            let parts: Vec<&str> = skill_id.split('/').collect();
            if parts.len() != 2 {
                return Err(HubError::ParseError(
                    "Invalid skill ID. Expected format: category/name".into(),
                ));
            }
            let (category, name) = (parts[0], parts[1]);
            let entry = hub.installer.install_from_market(category, name, force).await?;
            println!("Installed: {} v{}", entry.name, entry.version);
        }
        HubCommand::Sync { .. } => {
            let categories = hub.sync.sync_categories().await?;
            println!("Synced {} categories", categories.len());
        }
        HubCommand::List => {
            let skills = hub.index.list_skills()?;
            for skill in skills {
                println!("{}: {}", skill.id, skill.description);
            }
        }
        HubCommand::Uninstall { skill_id } => {
            hub.installer.uninstall(&skill_id)?;
            println!("Uninstalled: {}", skill_id);
        }
        HubCommand::View { skill_id } => {
            if let Some(skill) = hub.index.get_skill(&skill_id)? {
                println!("Name: {}", skill.name);
                println!("Category: {}", skill.category);
                println!("Version: {}", skill.version);
                println!("Description: {}", skill.description);
                println!("Installed at: {}", skill.installed_at);
            } else {
                return Err(HubError::SkillNotFound(skill_id));
            }
        }
        _ => {
            println!("Command not yet implemented");
        }
    }

    Ok(())
}
