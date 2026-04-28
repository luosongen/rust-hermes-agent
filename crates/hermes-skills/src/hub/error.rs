//! Hub 错误类型定义

use thiserror::Error;

/// Hub 操作错误
#[derive(Error, Debug)]
pub enum HubError {
    /// 技能未找到
    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    /// 技能已安装
    #[error("Skill already installed: {0}")]
    AlreadyInstalled(String),

    /// 下载失败
    #[error("Download failed: {0}")]
    DownloadFailed(String),

    /// 安全检查阻止
    #[error("Security blocked: {skill} - found {threats_len} threat(s)")]
    SecurityBlocked {
        skill: String,
        threats_len: usize,
    },

    /// 同步失败
    #[error("Sync failed: {0}")]
    SyncFailed(String),

    /// 索引错误
    #[error("Index error: {0}")]
    IndexError(String),

    /// 安装失败
    #[error("Install failed: {0}")]
    InstallFailed(String),

    /// 市场 API 错误
    #[error("Market API error: {0}")]
    MarketApiError(String),

    /// 解析错误
    #[error("Parse error: {0}")]
    ParseError(String),

    /// IO 错误
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// SQLite 错误
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    /// HTTP 请求错误
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}

impl HubError {
    /// 获取错误退出码
    pub fn exit_code(&self) -> i32 {
        match self {
            HubError::SkillNotFound(_) => 3,
            HubError::SecurityBlocked { .. } => 2,
            _ => 1,
        }
    }
}
