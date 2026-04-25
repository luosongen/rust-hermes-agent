//! Backup Types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 备份内容类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupType {
    /// 完整备份
    Full,
    /// 仅配置
    Config,
    /// 仅会话
    Sessions,
    /// 仅技能
    Skills,
    /// 仅内存/历史
    Memory,
}

impl BackupType {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Full,
            Self::Config,
            Self::Sessions,
            Self::Skills,
            Self::Memory,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Config => "config",
            Self::Sessions => "sessions",
            Self::Skills => "skills",
            Self::Memory => "memory",
        }
    }
}

/// 备份元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub id: String,
    pub backup_type: BackupType,
    pub created_at: DateTime<Utc>,
    pub version: String,
    pub description: Option<String>,
    pub size_bytes: u64,
    pub checksum: String,
    pub included_items: Vec<String>,
}

impl BackupMetadata {
    pub fn new(backup_type: BackupType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            backup_type,
            created_at: Utc::now(),
            version: "0.1.0".to_string(),
            description: None,
            size_bytes: 0,
            checksum: String::new(),
            included_items: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_size(mut self, size: u64) -> Self {
        self.size_bytes = size;
        self
    }

    pub fn with_checksum(mut self, checksum: &str) -> Self {
        self.checksum = checksum.to_string();
        self
    }

    pub fn add_item(&mut self, item: &str) {
        self.included_items.push(item.to_string());
    }
}

/// 备份内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupContents {
    pub metadata: BackupMetadata,
    pub config: Option<serde_json::Value>,
    pub sessions: Option<Vec<SessionBackup>>,
    pub skills: Option<Vec<SkillBackup>>,
    pub memory: Option<MemoryBackup>,
}

/// 会话备份
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBackup {
    pub session_id: String,
    pub messages: Vec<MessageBackup>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// 消息备份
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageBackup {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// 技能备份
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillBackup {
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub metadata: serde_json::Value,
}

/// 内存备份
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBackup {
    pub entries: Vec<MemoryEntry>,
    pub stats: MemoryStats,
}

/// 内存条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// 内存统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub categories: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_type_name() {
        assert_eq!(BackupType::Config.name(), "config");
        assert_eq!(BackupType::Full.name(), "full");
    }

    #[test]
    fn test_backup_metadata_new() {
        let meta = BackupMetadata::new(BackupType::Full);
        assert!(!meta.id.is_empty());
        assert_eq!(meta.backup_type, BackupType::Full);
    }

    #[test]
    fn test_backup_metadata_builder() {
        let meta = BackupMetadata::new(BackupType::Sessions)
            .with_description("Test backup")
            .with_size(1024)
            .with_checksum("abc123");

        assert_eq!(meta.description, Some("Test backup".to_string()));
        assert_eq!(meta.size_bytes, 1024);
        assert_eq!(meta.checksum, "abc123");
    }
}