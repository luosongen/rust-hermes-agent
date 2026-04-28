//! Hub 类型定义

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 技能来源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillSource {
    /// 本地技能
    Local,
    /// 远程技能
    Remote { url: String },
    /// Git 仓库
    Git { url: String, branch: String },
}

impl Default for SkillSource {
    fn default() -> Self {
        SkillSource::Local
    }
}

/// 技能索引条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexEntry {
    /// 技能 ID（格式：category/name）
    pub id: String,
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 所属分类
    pub category: String,
    /// 版本号
    pub version: String,
    /// 来源
    pub source: SkillSource,
    /// 校验和
    pub checksum: String,
    /// 文件路径
    pub file_path: String,
    /// 安装时间
    pub installed_at: DateTime<Utc>,
    /// 更新时间
    pub updated_at: DateTime<Utc>,
}

impl SkillIndexEntry {
    /// 创建新的技能索引条目
    pub fn new(id: String, name: String, category: String) -> Self {
        Self {
            id,
            name,
            description: String::new(),
            category,
            version: "1.0.0".to_string(),
            source: SkillSource::Local,
            checksum: String::new(),
            file_path: String::new(),
            installed_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

/// 技能分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    /// 分类名称
    pub name: String,
    /// 分类描述
    pub description: String,
    /// 图标
    pub icon: Option<String>,
    /// 技能数量
    pub skill_count: usize,
}

/// Hub 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    /// 默认 Hub URL
    pub default_hub: String,
    /// 自定义 Hub 列表
    pub custom_hubs: Vec<HubSource>,
    /// 同步间隔（秒）
    pub sync_interval_seconds: u64,
    /// 缓存 TTL（秒）
    pub cache_ttl_seconds: u64,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            default_hub: "https://market.hermes.dev".to_string(),
            custom_hubs: Vec::new(),
            sync_interval_seconds: 3600,
            cache_ttl_seconds: 86400,
        }
    }
}

/// Hub 源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSource {
    /// Hub 名称
    pub name: String,
    /// Hub URL
    pub url: String,
    /// API 密钥（可选）
    pub api_key: Option<String>,
}

/// 市场分类响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCategoriesResponse {
    /// 分类列表
    pub categories: Vec<MarketCategory>,
}

/// 市场分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCategory {
    /// 分类名称
    pub name: String,
    /// 分类描述
    pub description: String,
    /// 分类下的技能列表
    pub skills: Vec<MarketSkill>,
}

/// 市场技能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSkill {
    /// 技能 ID
    pub id: String,
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 版本号
    pub version: String,
    /// 下载 URL
    pub download_url: String,
    /// 校验和
    pub checksum: String,
}

/// Frontmatter 元数据
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    /// 技能名称
    pub name: Option<String>,
    /// 技能描述
    pub description: Option<String>,
    /// 所属分类
    pub category: Option<String>,
    /// 版本号
    pub version: Option<String>,
}
