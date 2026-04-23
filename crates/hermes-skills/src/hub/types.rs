use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// SkillSource enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SkillSource {
    Local,
    Remote { url: String },
    Git { url: String, branch: String },
}

impl Default for SkillSource {
    fn default() -> Self {
        SkillSource::Local
    }
}

// SkillIndexEntry struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIndexEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub version: String,
    pub source: SkillSource,
    pub checksum: String,
    pub file_path: String,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SkillIndexEntry {
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

// Category struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub skill_count: usize,
}

// HubConfig struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubConfig {
    pub default_hub: String,
    pub custom_hubs: Vec<HubSource>,
    pub sync_interval_seconds: u64,
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

// HubSource struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSource {
    pub name: String,
    pub url: String,
    pub api_key: Option<String>,
}

// Market response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCategoriesResponse {
    pub categories: Vec<MarketCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCategory {
    pub name: String,
    pub description: String,
    pub skills: Vec<MarketSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub download_url: String,
    pub checksum: String,
}
