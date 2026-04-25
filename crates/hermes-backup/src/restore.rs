//! Restore Module

use std::path::PathBuf;
use std::io::Read;
use flate2::read::GzDecoder;
use tar::Archive;
use sha2::{Sha256, Digest};

use crate::types::{BackupContents, BackupMetadata};
use crate::{BackupError, BackupResult};

/// 恢复管理器
pub struct RestoreManager {
    data_dir: PathBuf,
}

impl RestoreManager {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// 从备份文件恢复
    pub async fn restore_from_file(&self, backup_path: &PathBuf) -> BackupResult<RestoreReport> {
        // 读取备份文件
        let data = std::fs::read(backup_path)?;

        // 验证校验和
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let checksum = format!("{:x}", hasher.finalize());

        // 解压 - 使用 as_slice() 来满足 Read trait
        let decoder = GzDecoder::new(data.as_slice());
        let mut archive = Archive::new(decoder);

        let mut metadata: Option<BackupMetadata> = None;
        let mut contents: Option<BackupContents> = None;

        // 解包
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();

            if path == std::path::PathBuf::from("metadata.json") {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                let json_str = String::from_utf8(buf)
                    .map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
                metadata = Some(serde_json::from_str(&json_str)?);
            } else if path == std::path::PathBuf::from("contents.json") {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                let json_str = String::from_utf8(buf)
                    .map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
                contents = Some(serde_json::from_str(&json_str)?);
            }
        }

        let metadata = metadata.ok_or_else(|| BackupError::InvalidFormat("Missing metadata.json".to_string()))?;
        let contents = contents.ok_or_else(|| BackupError::InvalidFormat("Missing contents.json".to_string()))?;

        // 验证校验和
        if metadata.checksum != checksum {
            return Err(BackupError::InvalidFormat("Checksum mismatch".to_string()));
        }

        // 执行恢复
        self.restore(contents).await
    }

    /// 执行恢复
    pub async fn restore(&self, contents: BackupContents) -> BackupResult<RestoreReport> {
        let mut report = RestoreReport::new(contents.metadata.clone());
        let mut errors = Vec::new();

        // 恢复配置
        if let Some(config) = contents.config {
            let config_path = self.data_dir.join("config.json");
            if let Err(e) = self.write_json_file(&config_path, &config) {
                errors.push(format!("config: {}", e));
            } else {
                report.restored_items.push("config".to_string());
            }
        }

        // 恢复会话
        if let Some(sessions) = contents.sessions {
            let sessions_path = self.data_dir.join("sessions.json");
            if let Err(e) = self.write_json_file(&sessions_path, &sessions) {
                errors.push(format!("sessions: {}", e));
            } else {
                report.restored_items.push(format!("sessions ({} items)", sessions.len()));
            }
        }

        // 恢复技能
        if let Some(skills) = contents.skills {
            let skills_path = self.data_dir.join("skills");
            if let Err(e) = self.restore_skills(&skills_path, skills).await {
                errors.push(format!("skills: {}", e));
            } else {
                report.restored_items.push("skills".to_string());
            }
        }

        // 恢复内存
        if let Some(memory) = contents.memory {
            let memory_path = self.data_dir.join("memory.json");
            if let Err(e) = self.write_json_file(&memory_path, &memory) {
                errors.push(format!("memory: {}", e));
            } else {
                report.restored_items.push("memory".to_string());
            }
        }

        if !errors.is_empty() {
            report.errors = errors.clone();
        }

        Ok(report)
    }

    /// 恢复技能到目录
    async fn restore_skills(&self, skills_dir: &PathBuf, skills: Vec<crate::types::SkillBackup>) -> BackupResult<()> {
        std::fs::create_dir_all(skills_dir)?;

        for skill in skills {
            let skill_file = skills_dir.join(format!("{}.md", skill.name));
            let content = format!(
                "---\nname: {}\ndescription: {}\n---\n\n{}",
                skill.name,
                skill.description.unwrap_or_default(),
                skill.content
            );
            std::fs::write(&skill_file, content)?;
        }

        Ok(())
    }

    /// 写入 JSON 文件
    fn write_json_file(&self, path: &PathBuf, value: &impl serde::Serialize) -> BackupResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(value)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// 列出备份中的内容
    pub async fn inspect_backup(backup_path: &PathBuf) -> BackupResult<BackupMetadata> {
        let data = std::fs::read(backup_path)?;
        let decoder = GzDecoder::new(data.as_slice());
        let mut archive = Archive::new(decoder);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();

            if path == std::path::PathBuf::from("metadata.json") {
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf)?;
                let json_str = String::from_utf8(buf)
                    .map_err(|e| BackupError::InvalidFormat(e.to_string()))?;
                let metadata: BackupMetadata = serde_json::from_str(&json_str)?;
                return Ok(metadata);
            }
        }

        Err(BackupError::InvalidFormat("Missing metadata.json".to_string()))
    }
}

/// 恢复报告
#[derive(Debug)]
pub struct RestoreReport {
    pub metadata: BackupMetadata,
    pub restored_items: Vec<String>,
    pub errors: Vec<String>,
}

impl RestoreReport {
    pub fn new(metadata: BackupMetadata) -> Self {
        Self {
            metadata,
            restored_items: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BackupType;

    #[test]
    fn test_restore_report_success() {
        let metadata = BackupMetadata::new(BackupType::Full);
        let report = RestoreReport::new(metadata);
        assert!(report.is_success());
    }
}