//! 安装器模块
//!
//! 提供技能安装、卸载能力

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

/// 技能安装器
///
/// 负责从市场或 Git 仓库安装技能
pub struct Installer {
    /// 技能索引
    index: SkillIndex,
    /// 市场客户端
    market: MarketClient,
    /// 安全扫描器
    scanner: SecurityScanner,
    /// 技能目录
    pub skills_dir: PathBuf,
}

impl Installer {
    /// 创建安装器
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

    /// 从市场安装技能
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

    /// 从 Git 仓库安装技能
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

        // Create temp directory for clone
        let temp_dir = tempfile::tempdir()
            .map_err(|e| HubError::InstallFailed(format!("Temp dir failed: {}", e)))?;
        let temp_path = temp_dir.path();

        // Clone repository
        git_clone(git_url, branch, temp_path)
            .map_err(|e| HubError::InstallFailed(format!("Git clone failed: {}", e)))?;

        // Find all markdown files
        let md_files = find_markdown_files(temp_path);
        if md_files.is_empty() {
            return Err(HubError::InstallFailed(
                "No markdown files found in repository".to_string(),
            ));
        }

        // Process each markdown file
        let mut entries = Vec::new();
        for file_path in &md_files {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| HubError::IoError(e))?;

            // Parse frontmatter
            let (meta, _body) = parse_frontmatter(&content);

            // Determine skill name and category
            let skill_name = meta.name
                .as_ref()
                .map(|n| n.as_str())
                .unwrap_or(name);
            let skill_category = meta.category
                .as_ref()
                .map(|c| c.as_str())
                .unwrap_or(category);
            let skill_id = format!("{}/{}", skill_category, skill_name);

            // Skip if already installed (unless force)
            if !force {
                if let Some(existing) = self.index.get_skill(&skill_id)? {
                    continue; // Skip this file
                }
            }

            // Security scan
            let scan_result = self.scanner.scan(&content);
            if !scan_result.passed && !force {
                return Err(HubError::SecurityBlocked {
                    skill: skill_id.clone(),
                    threats_len: scan_result.threats.len(),
                });
            }

            // Calculate checksum
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let checksum = format!("sha256:{:x}", hasher.finalize());

            // Write to skills directory
            let category_dir = self.skills_dir.join(skill_category);
            std::fs::create_dir_all(&category_dir)?;
            let dest_path = category_dir.join(format!("{}.md", skill_name));
            std::fs::write(&dest_path, &content)?;

            // Create index entry
            let entry = SkillIndexEntry {
                id: skill_id.clone(),
                name: skill_name.to_string(),
                description: meta.description.unwrap_or_default(),
                category: skill_category.to_string(),
                version: meta.version.unwrap_or_else(|| "1.0.0".to_string()),
                source: SkillSource::Git {
                    url: git_url.to_string(),
                    branch: branch.to_string(),
                },
                checksum,
                file_path: dest_path.to_string_lossy().to_string(),
                installed_at: Utc::now(),
                updated_at: Utc::now(),
            };

            // Add to index
            self.index.add_skill(&entry)?;
            entries.push(entry);
        }

        // Return first entry (or error if none installed)
        entries.into_iter().next()
            .ok_or_else(|| HubError::InstallFailed("No skills installed".to_string()))
    }

    /// 卸载技能
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