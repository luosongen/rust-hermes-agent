//! 技能注册表模块
//!
//! 提供内存中的技能注册表，用于管理已加载的技能。
//!
//! ## 模块用途
//! - `SkillRegistry` 是技能加载后的内存存储容器
//! - 支持按名称精确查找、按名称或描述模糊搜索
//! - 防止同名技能重复注册
//!
//! ## 核心类型
//! - `SkillRegistry`: HashMap 实现的内存注册表
//!
//! ## 主要方法
//! - `register()`: 注册新技能，同名已存在时返回错误
//! - `get()`: 按名称精确查找技能
//! - `names()`: 列出所有已注册技能的名称
//! - `search()`: 按名称或描述的子串搜索技能（不区分大小写）
//! - `len()` / `is_empty()`: 查询注册表状态
//!
//! ## 与其他模块的关系
//! - `Skill` 类型来自 `loader` 模块
//! - `SkillError` 来自 `error` 模块

use crate::error::SkillError;
use crate::loader::Skill;
use std::collections::HashMap;

/// In-memory registry of loaded skills.
pub struct SkillRegistry {
    by_name: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    /// Register a skill. Returns error if a skill with the same name already exists.
    pub fn register(&mut self, skill: Skill) -> Result<(), SkillError> {
        let name = skill.metadata.name.clone();
        if self.by_name.contains_key(&name) {
            return Err(SkillError::AlreadyExists(name));
        }
        self.by_name.insert(name, skill);
        Ok(())
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.by_name.get(name)
    }

    /// List all skill names.
    pub fn names(&self) -> Vec<String> {
        self.by_name.keys().cloned().collect()
    }

    /// Search skills by name or description substring.
    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let query_lower = query.to_lowercase();
        self.by_name
            .values()
            .filter(|s| {
                s.metadata.name.to_lowercase().contains(&query_lower)
                    || s.metadata.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Total count of registered skills.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
