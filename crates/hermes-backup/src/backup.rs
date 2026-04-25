//! Backup Module

use std::path::PathBuf;
use flate2::{write::GzEncoder, Compression};
use tar::Header;
use sha2::{Sha256, Digest};

use crate::types::{BackupContents, BackupMetadata, BackupType};
use crate::{BackupError, BackupResult};

/// 备份构建器
pub struct BackupBuilder {
    backup_type: BackupType,
    description: Option<String>,
    config_path: Option<PathBuf>,
    sessions_path: Option<PathBuf>,
    skills_path: Option<PathBuf>,
    memory_path: Option<PathBuf>,
}

impl BackupBuilder {
    pub fn new(backup_type: BackupType) -> Self {
        Self {
            backup_type,
            description: None,
            config_path: None,
            sessions_path: None,
            skills_path: None,
            memory_path: None,
        }
    }

    pub fn with_config_path(mut self, path: PathBuf) -> Self {
        self.config_path = Some(path);
        self
    }

    pub fn with_sessions_path(mut self, path: PathBuf) -> Self {
        self.sessions_path = Some(path);
        self
    }

    pub fn with_skills_path(mut self, path: PathBuf) -> Self {
        self.skills_path = Some(path);
        self
    }

    pub fn with_memory_path(mut self, path: PathBuf) -> Self {
        self.memory_path = Some(path);
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// 构建备份
    pub async fn build(self) -> BackupResult<BackupContents> {
        let mut metadata = BackupMetadata::new(self.backup_type);
        if let Some(desc) = self.description {
            metadata = metadata.with_description(&desc);
        }

        let mut contents = BackupContents {
            metadata,
            config: None,
            sessions: None,
            skills: None,
            memory: None,
        };

        // 根据类型收集数据
        match self.backup_type {
            BackupType::Full | BackupType::Config => {
                if let Some(path) = self.config_path {
                    if path.exists() {
                        contents.config = Some(load_json_file(&path)?);
                        contents.metadata.add_item("config");
                    }
                }
            }
            _ => {}
        }

        match self.backup_type {
            BackupType::Full | BackupType::Sessions => {
                if let Some(path) = self.sessions_path {
                    if path.exists() {
                        contents.sessions = Some(load_json_file(&path)?);
                        contents.metadata.add_item("sessions");
                    }
                }
            }
            _ => {}
        }

        match self.backup_type {
            BackupType::Full | BackupType::Skills => {
                if let Some(path) = self.skills_path {
                    if path.exists() {
                        contents.skills = Some(load_json_file(&path)?);
                        contents.metadata.add_item("skills");
                    }
                }
            }
            _ => {}
        }

        match self.backup_type {
            BackupType::Full | BackupType::Memory => {
                if let Some(path) = self.memory_path {
                    if path.exists() {
                        contents.memory = Some(load_json_file(&path)?);
                        contents.metadata.add_item("memory");
                    }
                }
            }
            _ => {}
        }

        Ok(contents)
    }

    /// 打包为 tar.gz 并返回压缩数据
    pub async fn build_tarball(self) -> BackupResult<(Vec<u8>, BackupMetadata)> {
        let contents = self.build().await?;
        let mut metadata = contents.metadata.clone();

        let mut compressed_data = Vec::new();
        {
            let mut encoder = GzEncoder::new(&mut compressed_data, Compression::default());
            let mut tar_builder = tar::Builder::new(&mut encoder);

            // 添加 metadata.json
            let metadata_json = serde_json::to_string(&contents.metadata)?;
            let mut header = Header::new_gnu();
            header.set_path("metadata.json")?;
            header.set_size(metadata_json.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_builder.append(&header, metadata_json.as_bytes())?;

            // 添加 contents.json
            let contents_json = serde_json::to_string(&contents)?;
            let mut header = Header::new_gnu();
            header.set_path("contents.json")?;
            header.set_size(contents_json.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar_builder.append(&header, contents_json.as_bytes())?;

            tar_builder.finish()?;
            // encoder 在此隐式 finish
        }

        // 计算校验和
        let mut hasher = Sha256::new();
        hasher.update(&compressed_data);
        let checksum = format!("{:x}", hasher.finalize());

        metadata.size_bytes = compressed_data.len() as u64;
        metadata.checksum = checksum;

        Ok((compressed_data, metadata))
    }
}

/// 备份管理器
pub struct BackupManager {
    backup_dir: PathBuf,
}

impl BackupManager {
    pub fn new(backup_dir: PathBuf) -> Self {
        Self { backup_dir }
    }

    /// 创建备份
    pub async fn create_backup(&self, builder: BackupBuilder) -> BackupResult<PathBuf> {
        let (data, metadata) = builder.build_tarball().await?;

        // 生成文件名
        let filename = format!(
            "hermes-backup-{}-{}.tar.gz",
            metadata.backup_type.name(),
            metadata.id
        );
        let backup_path = self.backup_dir.join(&filename);

        // 确保目录存在
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 写入文件
        std::fs::write(&backup_path, &data)?;

        Ok(backup_path)
    }

    /// 列出所有备份
    pub async fn list_backups(&self) -> BackupResult<Vec<PathBuf>> {
        let mut backups = Vec::new();

        if !self.backup_dir.exists() {
            return Ok(backups);
        }

        let entries = std::fs::read_dir(&self.backup_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "gz" && path.file_name().map_or(false, |n| n.to_string_lossy().starts_with("hermes-backup-")) {
                    backups.push(path);
                }
            }
        }

        backups.sort_by(|a, b| b.cmp(a)); // 按修改时间倒序
        Ok(backups)
    }

    /// 删除备份
    pub async fn delete_backup(&self, backup_id: &str) -> BackupResult<()> {
        let backups = self.list_backups().await?;
        for backup in backups {
            if backup.file_name().map_or(false, |n| n.to_string_lossy().contains(backup_id)) {
                std::fs::remove_file(backup)?;
                return Ok(());
            }
        }
        Err(BackupError::NotFound(backup_id.to_string()))
    }
}

/// 辅助函数：加载 JSON 文件
fn load_json_file<T: serde::de::DeserializeOwned>(path: &PathBuf) -> BackupResult<T> {
    let content = std::fs::read_to_string(path)?;
    let value: T = serde_json::from_str(&content)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backup_builder_basic() {
        let builder = BackupBuilder::new(BackupType::Config);
        let contents = builder.build().await.unwrap();
        assert_eq!(contents.metadata.backup_type, BackupType::Config);
    }

    #[tokio::test]
    async fn test_backup_builder_with_description() {
        let builder = BackupBuilder::new(BackupType::Full)
            .with_description("Test backup");
        let contents = builder.build().await.unwrap();
        assert_eq!(contents.metadata.description, Some("Test backup".to_string()));
    }
}