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

/// 内存中的技能注册表
///
/// 使用 HashMap 存储已加载的技能，支持按名称查找和模糊搜索
pub struct SkillRegistry {
    by_name: HashMap<String, Skill>,
}

impl SkillRegistry {
    /// 创建空的技能注册表
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    /// 注册新技能
    ///
    /// 如果同名技能已存在，返回 `AlreadyExists` 错误
    pub fn register(&mut self, skill: Skill) -> Result<(), SkillError> {
        let name = skill.metadata.name.clone();
        if self.by_name.contains_key(&name) {
            return Err(SkillError::AlreadyExists(name));
        }
        self.by_name.insert(name, skill);
        Ok(())
    }

    /// 按名称精确查找技能
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.by_name.get(name)
    }

    /// 列出所有技能名称
    pub fn names(&self) -> Vec<String> {
        self.by_name.keys().cloned().collect()
    }

    /// 按名称或描述模糊搜索技能（不区分大小写）
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

    /// 已注册技能总数
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// 检查注册表是否为空
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// 列出所有技能
    pub fn list(&self) -> Vec<&Skill> {
        self.by_name.values().collect()
    }

    /// 更新已有技能（存在则替换）
    pub fn update(&mut self, skill: Skill) {
        let name = skill.metadata.name.clone();
        self.by_name.insert(name, skill);
    }

    /// 注销指定名称的技能
    pub fn unregister(&mut self, name: &str) -> Option<Skill> {
        self.by_name.remove(name)
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
