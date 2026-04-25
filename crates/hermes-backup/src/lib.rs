//! Hermes Backup System
//!
//! 提供配置、会话和技能的备份与恢复功能

pub mod backup;
pub mod restore;
pub mod types;

pub use backup::{BackupBuilder, BackupManager};
pub use restore::RestoreManager;
pub use types::{BackupContents, BackupMetadata, BackupType};

/// 备份错误类型
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Backup not found: {0}")]
    NotFound(String),
    #[error("Invalid backup format: {0}")]
    InvalidFormat(String),
    #[error("Partial restore failed: {0}")]
    PartialRestore(String),
}

pub type BackupResult<T> = Result<T, BackupError>;
