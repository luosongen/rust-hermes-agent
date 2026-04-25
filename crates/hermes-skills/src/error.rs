//! 技能模块错误类型定义
//!
//! 定义了技能加载、注册、执行过程中可能出现的所有错误。
//!
//! ## 错误类型
//! - `Io`: IO 错误（文件读取失败等），自动从 `std::io::Error` 转换
//! - `ParseFrontmatter`: frontmatter 解析错误（缺少分隔符、无效 YAML 等）
//! - `Yaml`: YAML 解析错误
//! - `NotFound`: 技能未找到
//! - `AlreadyExists`: 注册时发现同名技能已存在
//! - `InvalidPath`: 无效的技能文件路径
//! - `Download`: 技能下载失败
//! - `PlatformNotSupported`: 技能不支持当前平台
//!
//! ## 使用方式
//! 使用 `thiserror` 派生 `Error` trait，支持 `?` 运算符自动传播。

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkillError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse frontmatter: {0}")]
    ParseFrontmatter(String),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Skill not found: {0}")]
    NotFound(String),

    #[error("Skill already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid skill path: {0}")]
    InvalidPath(String),

    #[error("Download failed: {0}")]
    Download(String),

    #[error("Skill is disabled on this platform: {0}")]
    PlatformNotSupported(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Temp file error: {0}")]
    TempFile(String),

    #[error("Patch error: {0}")]
    Patch(String),
}

impl From<tempfile::PersistError> for SkillError {
    fn from(e: tempfile::PersistError) -> Self {
        SkillError::TempFile(e.to_string())
    }
}
