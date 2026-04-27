use chrono::Utc;
use sha2::{Sha256, Digest};
use std::path::PathBuf;
use crate::hub::error::HubError;
use crate::hub::index::SkillIndex;
use crate::hub::market::MarketClient;
use crate::hub::security::SecurityScanner;
use crate::hub::types::{SkillIndexEntry, SkillSource};

/// 从 git URL 克隆仓库（浅克隆）
fn git_clone(url: &str, _branch: &str, dest: &std::path::Path) -> Result<(), HubError> {
    git2::Repository::clone(url, dest)
        .map_err(|e| HubError::InstallFailed(format!("Git clone failed: {}", e)))?;
    Ok(())
}

/// 查找目录中所有 .md 文件
fn find_markdown_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// 解析 frontmatter
/// 返回 (Metadata, 正文内容)
fn parse_frontmatter(content: &str) -> (crate::hub::types::Metadata, &str) {
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let yaml_str = &content[3..end + 3];
            let body = content[end + 6..].trim();
            let meta: crate::hub::types::Metadata =
                serde_yaml::from_str(yaml_str).unwrap_or_default();
            return (meta, body);
        }
    }
    (crate::hub::types::Metadata::default(), content.trim())
}

pub struct Installer {
    index: SkillIndex,
    market: MarketClient,
    scanner: SecurityScanner,
    pub skills_dir: PathBuf,
}

impl Installer {
    pub fn new(
        index: SkillIndex,
        market: MarketClient,
        skills_dir: PathBuf,
    ) -> Self {
        Self {
            index,
            market,
            scanner: SecurityScanner::new(),
            skills_dir,
        }
    }

    pub async fn install_from_market(
        &self,
        category: &str,
        name: &str,
        force: bool,
    ) -> Result<SkillIndexEntry, HubError> {
        let id = format!("{}/{}", category, name);

        // Check if already installed
        if let Some(existing) = self.index.get_skill(&id)? {
            return Err(HubError::AlreadyInstalled(existing.id));
        }

        // Fetch skill metadata from market
        let market_skill = self.market.fetch_skill(category, name).await?;

        // Download skill content
        let content = self.market.download_skill(&market_skill.download_url).await?;

        // Security scan
        let scan_result = self.scanner.scan(&content);
        if !scan_result.passed && !force {
            return Err(HubError::SecurityBlocked {
                skill: id.clone(),
                threats_len: scan_result.threats.len(),
            });
        }

        // Calculate checksum
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let checksum = format!("sha256:{:x}", hasher.finalize());

        // Write to skills directory
        let category_dir = self.skills_dir.join(category);
        std::fs::create_dir_all(&category_dir)?;
        let file_path = category_dir.join(format!("{}.md", name));
        std::fs::write(&file_path, &content)?;

        // Create index entry
        let entry = SkillIndexEntry {
            id: id.clone(),
            name: market_skill.name,
            description: market_skill.description,
            category: category.to_string(),
            version: market_skill.version,
            source: SkillSource::Remote {
                url: market_skill.download_url,
            },
            checksum,
            file_path: file_path.to_string_lossy().to_string(),
            installed_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Add to index
        self.index.add_skill(&entry)?;

        Ok(entry)
    }

    pub async fn install_from_git(
        &self,
        git_url: &str,
        category: &str,
        name: &str,
        branch: &str,
        force: bool,
    ) -> Result<SkillIndexEntry, HubError> {
        let id = format!("{}/{}", category, name);

        // Check if already installed
        if let Some(existing) = self.index.get_skill(&id)? {
            return Err(HubError::AlreadyInstalled(existing.id));
        }

        // TODO: Implement git clone and extract
        // For now, return error indicating this is not yet implemented
        return Err(HubError::InstallFailed(
            "Git installation not yet implemented".to_string(),
        ));
    }

    pub fn uninstall(&self, id: &str) -> Result<(), HubError> {
        // Get skill entry
        let entry = self.index.get_skill(id)?
            .ok_or_else(|| HubError::SkillNotFound(id.to_string()))?;

        // Delete file
        let file_path = PathBuf::from(&entry.file_path);
        if file_path.exists() {
            std::fs::remove_file(file_path)?;
        }

        // Remove from index
        self.index.remove_skill(id)?;

        Ok(())
    }
}