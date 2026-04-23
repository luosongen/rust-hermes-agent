//! Hub error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum HubError {
    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Already installed: {0}")]
    AlreadyInstalled(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Security blocked: {skill} ({threats_len} threats)")]
    SecurityBlocked { skill: String, threats_len: usize },

    #[error("Sync failed: {0}")]
    SyncFailed(String),

    #[error("Index error: {0}")]
    IndexError(String),

    #[error("Install failed: {0}")]
    InstallFailed(String),

    #[error("Market API error: {0}")]
    MarketApiError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}

impl HubError {
    pub fn exit_code(&self) -> i32 {
        match self {
            HubError::SkillNotFound(_) => 3,
            HubError::SecurityBlocked { .. } => 2,
            _ => 1,
        }
    }
}
