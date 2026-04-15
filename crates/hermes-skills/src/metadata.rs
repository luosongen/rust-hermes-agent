//! 技能元数据模块
//!
//! 定义技能 Markdown 文件中 YAML frontmatter 的结构，并提供解析和查询方法。
//!
//! ## 核心类型
//! - `SkillMetadata`: 技能的主元数据（名称、描述、支持平台、hermes 特定配置）
//! - `HermesMetadata`: Hermes 特定的元数据（版本号、配置项、所需工具集）
//! - `SkillConfigItem`: 单个配置项的定义（键、描述、默认值）
//!
//! ## frontmatter 结构示例
//! ```yaml
//! name: my-skill
//! description: 这是一个示例技能
//! platforms: [cli, gateway]
//! metadata:
//!   version: "1.0"
//!   config:
//!     - key: api_endpoint
//!       description: API 端点地址
//!       default: https://api.example.com
//!   requires_toolsets: [http, filesystem]
//! ```
//!
//! ## 主要方法
//! - `SkillMetadata::supports_platform()`: 检查技能是否支持指定平台
//! - `SkillMetadata::requires_toolset()`: 检查技能是否需要指定工具集
//!
//! ## 与其他模块的关系
//! - `SkillLoader` 使用 `SkillMetadata` 解析 frontmatter
//! - 配置项由 `hermes_cli` 或 `hermes_gateway` 在运行时读取

use serde::{Deserialize, Serialize};

/// Configuration item defined in skill frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigItem {
    pub key: String,
    pub description: String,
    #[serde(default)]
    pub default: Option<String>,
}

/// Hermes-specific metadata inside the YAML frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HermesMetadata {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config: Vec<SkillConfigItem>,
    #[serde(default)]
    pub requires_toolsets: Vec<String>,
}

/// The YAML frontmatter of a skill file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub metadata: HermesMetadata,
}

impl SkillMetadata {
    /// Returns true if this skill supports the given platform.
    pub fn supports_platform(&self, platform: &str) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p == platform)
    }

    /// Returns true if the skill requires the given toolset.
    pub fn requires_toolset(&self, toolset: &str) -> bool {
        self.metadata
            .requires_toolsets
            .iter()
            .any(|t| t == toolset)
    }
}
