use clap::{Parser, Subcommand};
use crate::hub::{HubClient, HubError};
use std::path::PathBuf;

#[derive(Parser)]
pub struct HubCli {
    #[command(subcommand)]
    pub command: HubCommand,
}

#[derive(Subcommand)]
pub enum HubCommand {
    /// Browse available skills
    Browse {
        /// Specific category to browse
        #[arg(long)]
        category: Option<String>,
    },
    /// Search skills by name or description
    Search {
        /// Search query
        query: String,
    },
    /// Install a skill from market
    Install {
        /// Skill ID (e.g., software-development/writing-plans)
        skill_id: String,
        /// Skip security check
        #[arg(long)]
        force: bool,
    },
    /// Install from Git URL
    InstallFromGit {
        /// Git repository URL
        git_url: String,
        /// Category
        #[arg(long)]
        category: String,
        /// Skill name
        #[arg(long)]
        name: String,
        /// Branch
        #[arg(long, default_value = "main")]
        branch: String,
    },
    /// Sync market index
    Sync {
        /// Force refresh
        #[arg(long)]
        force: bool,
    },
    /// List installed skills
    List,
    /// Update a skill
    Update {
        skill_id: String,
    },
    /// Update all skills
    UpdateAll,
    /// Uninstall a skill
    Uninstall {
        skill_id: String,
    },
    /// View skill details
    View {
        skill_id: String,
    },
    /// View security scan results
    ViewSecurity {
        skill_id: String,
    },
    /// Trust a skill
    Trust {
        skill_id: String,
    },
    /// Remove trust from a skill
    Untrust {
        skill_id: String,
    },
}

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
