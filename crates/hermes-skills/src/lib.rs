//! Hermes Skills 技能管理模块
//!
//! 提供技能（Skill）的加载、注册、搜索和元数据管理能力。
//!
//! ## 模块划分
//! - `error`: 技能相关错误的定义（`SkillError`）
//! - `metadata`: 技能元数据结构（YAML frontmatter 解析）
//! - `loader`: 从文件系统加载技能（`Skill`、`SkillLoader`、`CodeBlock`）
//! - `registry`: 内存中的技能注册表（`SkillRegistry`）
//!
//! ## 核心类型
//! - `Skill`: 已加载的技能，包含元数据、正文内容、代码块和示例
//! - `SkillLoader`: 从本地目录加载所有技能
//! - `SkillRegistry`: 内存中的技能注册表，支持按名称查找和模糊搜索
//! - `SkillMetadata`: 解析自 YAML frontmatter 的技能元数据
//! - `SkillError`: 所有技能操作的错误类型
//!
//! ## 与其他模块的关系
//! - 被 `hermes_cli` 的 `skills` 子命令调用
//! - 技能文件格式为 Markdown，支持 YAML frontmatter 元数据
//! - 默认从 `~/.hermes/skills` 和 `./skills` 目录加载

pub mod error;
pub mod hub;
pub mod hub_cli;
pub mod fuzzy_patch;
pub mod loader;
pub mod metadata;
pub mod registry;
pub mod security;
pub mod tools;
pub mod manager;
pub mod executor;

#[cfg(test)]
mod tests;

pub use error::SkillError;
pub use loader::{CodeBlock, Skill, SkillLoader};
pub use metadata::{HermesMetadata, SkillConfigItem, SkillMetadata};
pub use registry::SkillRegistry;
pub use security::{SecurityScanResult, SecurityThreat, scan_content};
pub use hub::{HubClient, HubError, HubConfig, HubSource, SkillIndex, SkillIndexEntry, Category};
pub use hub::{MarketClient, Installer, Sync, Browse};
pub use hub::{SecurityScanner, Severity, ThreatType};
pub use hub_cli::{HubCli, HubCommand, run_hub_command};
pub use tools::{skills_list, skills_view, skills_manage, SkillListItem, SkillViewResult, SkillsListArgs, SkillsViewArgs, SkillsManageArgs};
pub use manager::{SkillManager, CreateResult, EditResult, PatchResult, DeleteResult, WriteFileResult, RemoveFileResult};
