//! Skills command handlers
//!
//! Provides handlers for the `skills` subcommand: list, search, install, uninstall.

use anyhow::Result;
use hermes_skills::{SkillLoader, SkillRegistry};
use std::path::PathBuf;

/// Default skills directory: `~/.hermes/skills`
fn default_skills_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hermes")
        .join("skills")
}

/// Load the skill registry from the default skills directory.
fn load_registry() -> Result<SkillRegistry, hermes_skills::SkillError> {
    let loader = SkillLoader::new(vec![default_skills_dir()]);
    let skills = loader.load_all()?;
    let mut registry = SkillRegistry::new();
    for skill in skills {
        registry.register(skill)?;
    }
    Ok(registry)
}

/// List all installed skills.
pub fn list_skills() -> Result<()> {
    let registry = load_registry()?;
    let names = registry.names();
    if names.is_empty() {
        println!("No skills installed.");
    } else {
        println!("Installed skills ({}):", names.len());
        for name in names {
            if let Some(skill) = registry.get(&name) {
                println!("  - {}: {}", name, skill.metadata.description);
            }
        }
    }
    Ok(())
}

/// Search skills by name or description.
pub fn search_skills(query: &str) -> Result<()> {
    let registry = load_registry()?;
    let results = registry.search(query);
    if results.is_empty() {
        println!("No skills found matching '{}'.", query);
    } else {
        println!("Found {} skill(s):", results.len());
        for skill in results {
            println!("  - {}: {}", skill.metadata.name, skill.metadata.description);
        }
    }
    Ok(())
}

/// Install a skill from a source (stub).
pub fn install_skill(skill_source: &str) -> Result<()> {
    println!(
        "Install skill from '{}' (not yet implemented).",
        skill_source
    );
    println!("Hint: Use a skill market or provide a git URL / file path.");
    Ok(())
}

/// Uninstall a skill by removing its directory.
pub fn uninstall_skill(skill_name: &str) -> Result<()> {
    let skills_dir = default_skills_dir();
    let skill_path = skills_dir.join(skill_name);
    if !skill_path.exists() {
        println!("Skill '{}' not found at {:?}", skill_name, skill_path);
        return Ok(());
    }
    if skill_path.is_dir() {
        std::fs::remove_dir_all(&skill_path)?;
    } else {
        std::fs::remove_file(&skill_path)?;
    }
    println!("Skill '{}' uninstalled.", skill_name);
    Ok(())
}
