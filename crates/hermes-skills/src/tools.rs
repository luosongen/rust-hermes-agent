//! Skills tools - skills_list, skills_view, skills_manage
//!
//! Provides tool functions for skill management:
//! - `skills_list`: List all skills with metadata
//! - `skills_view`: View full skill content
//! - `skills_manage`: CRUD for skill self-improvement

use crate::error::SkillError;
use crate::loader::SkillLoader;
use crate::registry::SkillRegistry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SkillsListArgs {
    #[allow(dead_code)]
    pub category: Option<String>, // Reserved for future use
}

#[derive(Debug, Deserialize)]
pub struct SkillsViewArgs {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SkillsManageArgs {
    pub action: String, // "create" | "edit" | "patch" | "delete"
    pub name: String,
    pub content: Option<String>,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SkillListItem {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct SkillViewResult {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// Tool: skills_list - List all available skills
pub fn skills_list(registry: &SkillRegistry, _args: SkillsListArgs) -> Result<Vec<SkillListItem>, SkillError> {
    let skills = registry.list();
    let mut items: Vec<SkillListItem> = skills
        .iter()
        .map(|s| SkillListItem {
            name: s.metadata.name.clone(),
            description: s.metadata.description.clone(),
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Tool: skills_view - View full skill content
pub fn skills_view(registry: &SkillRegistry, args: SkillsViewArgs) -> Result<SkillViewResult, SkillError> {
    let skill = registry
        .get(&args.name)
        .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", args.name)))?;

    Ok(SkillViewResult {
        name: skill.metadata.name.clone(),
        description: skill.metadata.description.clone(),
        content: skill.content.clone(),
    })
}

/// Tool: skills_manage - CRUD for skill self-improvement
pub fn skills_manage(
    registry: &mut SkillRegistry,
    skills_dir: &std::path::Path,
    args: SkillsManageArgs,
) -> Result<String, SkillError> {
    match args.action.as_str() {
        "create" => {
            let content = args
                .content
                .ok_or_else(|| SkillError::InvalidInput("content required".to_string()))?;
            let skill = SkillLoader::parse_skill_content(&args.name, &content)?;
            let skill_path = skills_dir.join(&args.name).join("SKILL.md");
            if skill_path.exists() {
                return Err(SkillError::AlreadyExists(args.name));
            }
            std::fs::create_dir_all(skill_path.parent().unwrap())?;
            std::fs::write(&skill_path, &content)?;
            registry.register(skill)?;
            Ok(format!("Skill '{}' created", args.name))
        }
        "edit" => {
            let content = args
                .content
                .ok_or_else(|| SkillError::InvalidInput("content required".to_string()))?;
            let skill = SkillLoader::parse_skill_content(&args.name, &content)?;
            let skill_path = skills_dir.join(&args.name).join("SKILL.md");
            if !skill_path.exists() {
                return Err(SkillError::NotFound(format!("Skill '{}' not found", args.name)));
            }
            std::fs::write(&skill_path, &content)?;
            registry.update(skill);
            Ok(format!("Skill '{}' updated", args.name))
        }
        "patch" => {
            let old_string = args
                .old_string
                .ok_or_else(|| SkillError::InvalidInput("old_string required".to_string()))?;
            let new_string = args
                .new_string
                .ok_or_else(|| SkillError::InvalidInput("new_string required".to_string()))?;
            let skill_path = skills_dir.join(&args.name).join("SKILL.md");
            if !skill_path.exists() {
                return Err(SkillError::NotFound(format!("Skill '{}' not found", args.name)));
            }
            let content = std::fs::read_to_string(&skill_path)?;
            let fuzzy_patch = crate::fuzzy_patch::FuzzyPatch::new();
            let patched = fuzzy_patch
                .patch(&content, &old_string, &new_string)
                .map_err(SkillError::InvalidInput)?;
            std::fs::write(&skill_path, &patched)?;
            let new_content = std::fs::read_to_string(&skill_path)?;
            let skill = SkillLoader::parse_skill_content(&args.name, &new_content)?;
            registry.update(skill);
            Ok(format!("Skill '{}' patched", args.name))
        }
        "delete" => {
            let skill_path = skills_dir.join(&args.name);
            if !skill_path.exists() {
                return Err(SkillError::NotFound(format!("Skill '{}' not found", args.name)));
            }
            std::fs::remove_dir_all(&skill_path)?;
            registry.unregister(&args.name);
            Ok(format!("Skill '{}' deleted", args.name))
        }
        _ => Err(SkillError::InvalidInput(format!(
            "Unknown action: {}",
            args.action
        ))),
    }
}