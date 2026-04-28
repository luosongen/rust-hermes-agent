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

/// 技能配置项，定义在 YAML frontmatter 中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigItem {
    /// 配置项键名
    pub key: String,
    /// 配置项描述
    pub description: String,
    /// 默认值（可选）
    #[serde(default)]
    pub default: Option<String>,
}

/// Hermes 特定元数据，嵌套在 YAML frontmatter 的 metadata 字段中
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HermesMetadata {
    /// 技能版本号
    #[serde(default)]
    pub version: Option<String>,
    /// 配置项列表
    #[serde(default)]
    pub config: Vec<SkillConfigItem>,
    /// 所需工具集列表
    #[serde(default)]
    pub requires_toolsets: Vec<String>,
}

/// 技能 YAML frontmatter 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// 技能名称（唯一标识）
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 支持的平台列表（如 cli、gateway）
    #[serde(default)]
    pub platforms: Vec<String>,
    /// Hermes 特定元数据
    #[serde(default)]
    pub metadata: HermesMetadata,
}

impl SkillMetadata {
    /// 检查技能是否支持指定平台
    ///
    /// 如果 platforms 为空，则默认支持所有平台
    pub fn supports_platform(&self, platform: &str) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p == platform)
    }

    /// 检查技能是否需要指定工具集
    pub fn requires_toolset(&self, toolset: &str) -> bool {
        self.metadata
            .requires_toolsets
            .iter()
            .any(|t| t == toolset)
    }
}
