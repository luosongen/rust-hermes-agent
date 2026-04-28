//! 技能工具模块 - skills_list, skills_view, skills_manage
//!
//! 提供技能管理的工具函数：
//! - `skills_list`: 列出所有技能及其元数据
//! - `skills_view`: 查看技能完整内容
//! - `skills_manage`: 技能自我改进的 CRUD 操作

use crate::error::SkillError;
use crate::loader::SkillLoader;
use crate::registry::SkillRegistry;
use serde::{Deserialize, Serialize};

/// skills_list 工具参数
#[derive(Debug, Deserialize)]
pub struct SkillsListArgs {
    /// 分类筛选（保留用于未来扩展）
    #[allow(dead_code)]
    pub category: Option<String>,
}

/// skills_view 工具参数
#[derive(Debug, Deserialize)]
pub struct SkillsViewArgs {
    /// 技能名称
    pub name: String,
    /// 文件路径（可选）
    pub file_path: Option<String>,
}

/// skills_manage 工具参数
#[derive(Debug, Deserialize)]
pub struct SkillsManageArgs {
    /// 操作类型：create | edit | patch | delete
    pub action: String,
    /// 技能名称
    pub name: String,
    /// 完整内容（用于 create/edit）
    pub content: Option<String>,
    /// 旧字符串（用于 patch）
    pub old_string: Option<String>,
    /// 新字符串（用于 patch）
    pub new_string: Option<String>,
}

/// 技能列表项
#[derive(Debug, Serialize)]
pub struct SkillListItem {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 所属分类
    pub category: String,
}

/// 技能查看结果
#[derive(Debug, Serialize)]
pub struct SkillViewResult {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 技能内容
    pub content: String,
}

/// 工具：skills_list - 列出所有可用技能
pub fn skills_list(registry: &SkillRegistry, args: SkillsListArgs) -> Result<Vec<SkillListItem>, SkillError> {
    let skills = registry.list();
    let mut items: Vec<SkillListItem> = skills
        .iter()
        .filter(|s| {
            if let Some(ref cat) = args.category {
                s.metadata.platforms.iter().any(|p| p == cat)
            } else {
                true
            }
        })
        .map(|s| SkillListItem {
            name: s.metadata.name.clone(),
            description: s.metadata.description.clone(),
            category: s.metadata.platforms.join(","),
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// 工具：skills_view - 查看技能完整内容
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

/// 工具：skills_manage - 技能自我改进的 CRUD 操作
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