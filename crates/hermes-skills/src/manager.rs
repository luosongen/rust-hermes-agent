//! Skill Manager — Agent 自主管理技能的逻辑
//!
//! 提供 SkillManager 结构体，实现技能的创建、编辑、补丁、删除和文件操作。

use crate::error::SkillError;
use crate::fuzzy_patch::FuzzyPatch;
use crate::loader::SkillLoader;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// 验证规则常量
const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;
const MAX_SKILL_CONTENT_CHARS: usize = 100_000;
const MAX_SUPPORT_FILE_BYTES: usize = 1_048_576;

/// 允许的子目录
const ALLOWED_SUBDIRS: &[&str] = &["references", "templates", "scripts", "assets"];

/// Skill 名称验证正则
const VALID_NAME_RE: &str = r"^[a-z0-9][a-z0-9._-]*$";

/// SkillManager 处理所有技能管理操作
#[derive(Clone)]
pub struct SkillManager {
    skills_dir: PathBuf,
    fuzzy_patch: FuzzyPatch,
}

impl SkillManager {
    /// 使用默认 skills 目录创建
    pub fn new() -> Result<Self, SkillError> {
        let skills_dir = Self::default_skills_dir()?;
        Ok(Self::with_dir(skills_dir))
    }

    /// 使用指定目录创建
    pub fn with_dir(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            fuzzy_patch: FuzzyPatch::new(),
        }
    }

    fn default_skills_dir() -> Result<PathBuf, SkillError> {
        dirs::home_dir()
            .map(|h| h.join(".hermes/skills"))
            .ok_or_else(|| SkillError::InvalidPath("Cannot find home directory".into()))
    }

    /// 获取 skills 根目录
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    /// 验证 skill 名称
    pub fn validate_name(name: &str) -> Result<(), SkillError> {
        if name.is_empty() {
            return Err(SkillError::InvalidInput("Skill name is required.".into()));
        }
        if name.len() > MAX_NAME_LENGTH {
            return Err(SkillError::InvalidInput(
                format!("Skill name exceeds {} characters.", MAX_NAME_LENGTH)
            ));
        }
        let re = regex::Regex::new(VALID_NAME_RE).unwrap();
        if !re.is_match(name) {
            return Err(SkillError::InvalidInput(
                "Invalid skill name. Use lowercase letters, numbers, hyphens, dots, and underscores.".into()
            ));
        }
        Ok(())
    }

    /// 验证 category
    pub fn validate_category(category: &str) -> Result<(), SkillError> {
        if category.is_empty() {
            return Ok(());
        }
        if category.len() > MAX_NAME_LENGTH {
            return Err(SkillError::InvalidInput("Category exceeds maximum length.".into()));
        }
        let re = regex::Regex::new(VALID_NAME_RE).unwrap();
        if !re.is_match(category) {
            return Err(SkillError::InvalidInput(
                "Invalid category name.".into()
            ));
        }
        Ok(())
    }

    /// 验证 frontmatter 内容
    pub fn validate_frontmatter(content: &str) -> Result<(), SkillError> {
        if content.trim().is_empty() {
            return Err(SkillError::InvalidInput("Content cannot be empty.".into()));
        }
        if !content.starts_with("---") {
            return Err(SkillError::InvalidInput(
                "SKILL.md must start with YAML frontmatter (---).".into()
            ));
        }
        // Parse and validate required fields
        let (_, body) = crate::loader::Skill::parse_frontmatter(content)
            .map_err(|e| SkillError::InvalidInput(format!("Frontmatter error: {}", e)))?;
        if body.trim().is_empty() {
            return Err(SkillError::InvalidInput(
                "SKILL.md must have content after the frontmatter.".into()
            ));
        }
        Ok(())
    }

    /// 验证 file_path 不允许路径遍历
    pub fn validate_file_path(file_path: &str) -> Result<(), SkillError> {
        if file_path.contains("..") {
            return Err(SkillError::InvalidInput("Path traversal ('..') is not allowed.".into()));
        }
        let first_dir = file_path.split('/').next().unwrap_or("");
        if !ALLOWED_SUBDIRS.contains(&first_dir) {
            return Err(SkillError::InvalidInput(
                format!("File must be under one of: {}.", ALLOWED_SUBDIRS.join(", "))
            ));
        }
        Ok(())
    }

    /// 解析 skill 路径
    fn resolve_skill_dir(&self, name: &str, category: Option<&str>) -> PathBuf {
        match category {
            Some(cat) => self.skills_dir.join(cat).join(name),
            None => self.skills_dir.join(name),
        }
    }

    /// 原子性写入文件
    fn atomic_write(path: &Path, content: &str) -> Result<(), SkillError> {
        let parent = path.parent().ok_or_else(||
            SkillError::InvalidPath("Cannot determine parent directory".into())
        )?;
        fs::create_dir_all(parent)?;

        let mut temp_file = NamedTempFile::new_in(parent)?;
        std::io::Write::write_all(&mut temp_file, content.as_bytes())?;
        temp_file.persist(path)?;
        Ok(())
    }

    /// 查找 skill 目录
    fn find_skill_dir(&self, name: &str) -> Option<PathBuf> {
        // 直接检查顶层
        let direct = self.skills_dir.join(name);
        if direct.exists() {
            return Some(direct);
        }
        // 搜索所有可能的路径（category 目录）
        if let Ok(entries) = fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let skill_dir = path.join(name);
                    if skill_dir.exists() {
                        return Some(skill_dir);
                    }
                }
            }
        }
        None
    }

    /// 创建新 skill
    pub fn create(&self, name: &str, content: &str, category: Option<&str>) -> Result<CreateResult, SkillError> {
        // 验证
        Self::validate_name(name)?;
        if let Some(cat) = category {
            Self::validate_category(cat)?;
        }
        Self::validate_frontmatter(content)?;

        // 检查是否已存在
        let skill_dir = self.resolve_skill_dir(name, category);
        if skill_dir.exists() {
            return Err(SkillError::AlreadyExists(name.into()));
        }

        // 创建目录结构
        fs::create_dir_all(&skill_dir)?;
        for subdir in ALLOWED_SUBDIRS {
            fs::create_dir_all(skill_dir.join(subdir))?;
        }

        // 写入 SKILL.md
        let skill_md = skill_dir.join("SKILL.md");
        Self::atomic_write(&skill_md, content)?;

        Ok(CreateResult {
            success: true,
            message: format!("Skill '{}' created.", name),
            path: skill_dir.to_string_lossy().into_owned(),
            category: category.map(String::from),
        })
    }

    /// 编辑现有 skill
    pub fn edit(&self, name: &str, content: &str) -> Result<EditResult, SkillError> {
        Self::validate_frontmatter(content)?;

        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        let skill_md = skill_dir.join("SKILL.md");
        Self::atomic_write(&skill_md, content)?;

        Ok(EditResult {
            success: true,
            message: format!("Skill '{}' updated.", name),
            path: skill_dir.to_string_lossy().into_owned(),
        })
    }

    /// 补丁修改 skill
    pub fn patch(&self, name: &str, old_string: &str, new_string: &str, replace_all: bool, file_path: Option<&str>) -> Result<PatchResult, SkillError> {
        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        let target = match file_path {
            Some(fp) => {
                Self::validate_file_path(fp)?;
                skill_dir.join(fp)
            }
            None => skill_dir.join("SKILL.md"),
        };

        if !target.exists() {
            return Err(SkillError::NotFound(format!("File not found: {:?}", target)));
        }

        let content = fs::read_to_string(&target)?;

        let patched_content = if replace_all {
            // 全部替换
            if !content.contains(old_string) {
                return Err(SkillError::InvalidInput("old_string not found in content".into()));
            }
            content.replace(old_string, new_string)
        } else {
            // 精确匹配一次
            self.fuzzy_patch.patch(&content, old_string, new_string)?
        };

        // 验证 frontmatter 仍然有效（如果是 SKILL.md）
        if target.extension().map(|e| e == "md").unwrap_or(false) && file_path.is_none() {
            Self::validate_frontmatter(&patched_content)?;
        }

        Self::atomic_write(&target, &patched_content)?;

        Ok(PatchResult {
            success: true,
            message: format!("Patched in skill '{}'.", name),
            match_count: 1,
        })
    }

    /// 删除 skill
    pub fn delete(&self, name: &str) -> Result<DeleteResult, SkillError> {
        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        fs::remove_dir_all(&skill_dir)?;

        // 清理空 category 目录
        if let Some(parent) = skill_dir.parent() {
            if parent != self.skills_dir && parent.exists() {
                if fs::read_dir(parent)?.next().is_none() {
                    fs::remove_dir(parent)?;
                }
            }
        }

        Ok(DeleteResult {
            success: true,
            message: format!("Skill '{}' deleted.", name),
        })
    }

    /// 写入支持文件
    pub fn write_file(&self, name: &str, file_path: &str, file_content: &str) -> Result<WriteFileResult, SkillError> {
        Self::validate_file_path(file_path)?;

        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        // 检查文件大小
        if file_content.len() > MAX_SUPPORT_FILE_BYTES {
            return Err(SkillError::InvalidInput(
                format!("File content exceeds {} bytes limit.", MAX_SUPPORT_FILE_BYTES)
            ));
        }

        let target = skill_dir.join(file_path);
        Self::atomic_write(&target, file_content)?;

        Ok(WriteFileResult {
            success: true,
            message: format!("File '{}' written to skill '{}'.", file_path, name),
            path: target.to_string_lossy().into_owned(),
        })
    }

    /// 删除支持文件
    pub fn remove_file(&self, name: &str, file_path: &str) -> Result<RemoveFileResult, SkillError> {
        Self::validate_file_path(file_path)?;

        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        let target = skill_dir.join(file_path);
        if !target.exists() {
            return Err(SkillError::NotFound(format!("File '{}' not found", file_path)));
        }

        fs::remove_file(&target)?;

        // 清理空子目录
        if let Some(parent) = target.parent() {
            if parent != skill_dir && parent.exists() {
                if fs::read_dir(parent)?.next().is_none() {
                    fs::remove_dir(parent)?;
                }
            }
        }

        Ok(RemoveFileResult {
            success: true,
            message: format!("File '{}' removed from skill '{}'.", file_path, name),
        })
    }
}

#[derive(serde::Serialize)]
pub struct CreateResult {
    pub success: bool,
    pub message: String,
    pub path: String,
    pub category: Option<String>,
}

#[derive(serde::Serialize)]
pub struct EditResult {
    pub success: bool,
    pub message: String,
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct PatchResult {
    pub success: bool,
    pub message: String,
    pub match_count: usize,
}

#[derive(serde::Serialize)]
pub struct DeleteResult {
    pub success: bool,
    pub message: String,
}

#[derive(serde::Serialize)]
pub struct WriteFileResult {
    pub success: bool,
    pub message: String,
    pub path: String,
}

#[derive(serde::Serialize)]
pub struct RemoveFileResult {
    pub success: bool,
    pub message: String,
}
