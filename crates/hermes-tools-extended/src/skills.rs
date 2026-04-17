//! SkillsTool — Skills 管理工具
//!
//! 提供 list / view / search / sync / install / remove 操作。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

const SKILLS_DIR: &str = ".config/hermes-agent/skills";
const MANIFEST_FILE: &str = ".bundled_manifest";
const SKILL_FILE: &str = "SKILL.md";
const SKILLS_API_URL: &str = "https://skills.sh";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub origin_hash: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillManifestEntry {
    pub source: String,
    pub origin_hash: String,
    #[serde(default)]
    pub installed_at: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BundledManifest {
    pub version: u32,
    pub skills: std::collections::HashMap<String, SkillManifestEntry>,
}

impl Default for BundledManifest {
    fn default() -> Self {
        Self { version: 1, skills: std::collections::HashMap::new() }
    }
}

#[derive(Clone)]
pub struct SkillsTool {
    skills_dir: PathBuf,
    http_client: reqwest::Client,
}

impl SkillsTool {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            skills_dir: PathBuf::from(home).join(SKILLS_DIR),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.skills_dir.join(MANIFEST_FILE)
    }

    async fn ensure_dir(&self) -> Result<(), ToolError> {
        tokio::fs::create_dir_all(&self.skills_dir).await
            .map_err(|e| ToolError::Execution(format!("failed to create skills dir: {}", e)))
    }

    async fn read_manifest(&self) -> Result<BundledManifest, ToolError> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(BundledManifest::default());
        }
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| ToolError::Execution(format!("failed to read manifest: {}", e)))?;
        serde_json::from_str(&content)
            .map_err(|e| ToolError::Execution(format!("failed to parse manifest: {}", e)))
    }

    async fn write_manifest(&self, manifest: &BundledManifest) -> Result<(), ToolError> {
        let content = serde_json::to_string_pretty(manifest)
            .map_err(|e| ToolError::Execution(format!("failed to serialize manifest: {}", e)))?;
        tokio::fs::write(&self.manifest_path(), content).await
            .map_err(|e| ToolError::Execution(format!("failed to write manifest: {}", e)))
    }

    /// 从 SKILL.md 内容中解析 frontmatter 和正文
    /// 返回 (metadata, content_preview)
    pub fn parse_skill_markdown(content: &str) -> Option<(SkillMetadata, String)> {
        let trimmed = content.trim();
        if !trimmed.starts_with("---") {
            return None;
        }
        let second_dash = trimmed[3..].find("---")?;
        let yaml_str = &trimmed[3..second_dash + 3];
        let metadata: SkillMetadata = serde_yaml::from_str(yaml_str).ok()?;
        let after_second = &trimmed[second_dash + 6..];
        let preview = if after_second.len() > 200 {
            format!("{}...", &after_second[..200])
        } else {
            after_second.to_string()
        };
        Some((metadata, preview))
    }

    /// 读取本地 skill 的元信息
    async fn read_local_skill(&self, name: &str) -> Result<Option<(SkillMetadata, String)>, ToolError> {
        let skill_path = self.skills_dir.join(name).join(SKILL_FILE);
        if !skill_path.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&skill_path).await
            .map_err(|e| ToolError::Execution(format!("failed to read skill file: {}", e)))?;
        Ok(Self::parse_skill_markdown(&content).map(|(m, p)| (m, p)))
    }

    /// 列出本地所有已安装的 skills
    async fn list_local(&self) -> Result<Vec<SkillMetadata>, ToolError> {
        self.ensure_dir().await?;
        let mut entries = tokio::fs::read_dir(&self.skills_dir).await
            .map_err(|e| ToolError::Execution(format!("failed to read skills dir: {}", e)))?;
        let mut results = Vec::new();
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ToolError::Execution(format!("dir read error: {}", e)))? {
            let path = entry.path();
            let name = match path.file_name() {
                Some(n) => n.to_string_lossy(),
                None => continue,
            };
            // Skip hidden directories like .bundled_manifest
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                let skill_md = path.join(SKILL_FILE);
                if skill_md.exists() {
                    if let Ok(content) = tokio::fs::read_to_string(&skill_md).await {
                        if let Some((meta, _)) = Self::parse_skill_markdown(&content) {
                            results.push(meta);
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    /// 从远程搜索 skills
    async fn search_remote(&self, query: &str, limit: usize) -> Result<Vec<serde_json::Value>, ToolError> {
        let url = format!("{}?query={}&limit={}", SKILLS_API_URL, urlencoding::encode(query), limit);
        let resp = self.http_client.get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("search failed: {}", e)))?;
        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("invalid search response: {}", e)))?;
        let skills = body.get("skills")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(skills)
    }

    /// 下载并安装一个 skill
    async fn install_skill(&self, name: &str, source: &str) -> Result<(), ToolError> {
        self.ensure_dir().await?;

        let skill_dir = self.skills_dir.join(name);
        if skill_dir.join(SKILL_FILE).exists() {
            return Err(ToolError::Execution(format!("skill '{}' already installed", name)));
        }

        // 下载 SKILL.md
        let resp = self.http_client.get(source)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("download failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ToolError::Execution(format!("HTTP {}", resp.status())));
        }

        let content = resp.text().await
            .map_err(|e| ToolError::Execution(format!("read response: {}", e)))?;

        // 计算 hash
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        // 创建目录并写入文件
        tokio::fs::create_dir_all(&skill_dir).await
            .map_err(|e| ToolError::Execution(format!("create dir failed: {}", e)))?;
        tokio::fs::write(skill_dir.join(SKILL_FILE), &content).await
            .map_err(|e| ToolError::Execution(format!("write skill file: {}", e)))?;

        // 更新 manifest
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64;
        let mut manifest = self.read_manifest().await?;
        manifest.skills.insert(name.to_string(), SkillManifestEntry {
            source: source.to_string(),
            origin_hash: hash,
            installed_at: now,
        });
        self.write_manifest(&manifest).await?;

        Ok(())
    }
}

#[async_trait]
impl Tool for SkillsTool {
    fn name(&self) -> &str { "skills" }
    fn description(&self) -> &str {
        "Manage local and remote AI skills. Actions: list, view, search, sync, install, remove."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {"properties": {"action": {"const": "list"}}, "required": ["action"]},
                {"properties": {"action": {"const": "view"}, "name": {"type": "string"}}, "required": ["action", "name"]},
                {"properties": {"action": {"const": "search"}, "query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["action", "query"]},
                {"properties": {"action": {"const": "sync"}}, "required": ["action"]},
                {"properties": {"action": {"const": "install"}, "name": {"type": "string"}, "source": {"type": "string"}}, "required": ["action", "name"]},
                {"properties": {"action": {"const": "remove"}, "name": {"type": "string"}}, "required": ["action", "name"]}
            ]
        })
    }
    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        #[derive(Deserialize)]
        #[serde(tag = "action", rename_all = "lowercase")]
        enum SkillAction {
            List,
            View { name: String },
            Search { query: String, #[serde(default)] limit: Option<usize> },
            Sync,
            Install { name: String, #[serde(default)] source: Option<String> },
            Remove { name: String },
        }

        let params: SkillAction = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params {
            SkillAction::List => {
                let skills = self.list_local().await?;
                Ok(json!({ "skills": skills }).to_string())
            }
            SkillAction::View { name } => {
                let manifest = self.read_manifest().await?;
                if !manifest.skills.contains_key(&name) && !self.skills_dir.join(&name).join(SKILL_FILE).exists() {
                    return Err(ToolError::Execution(format!("skill '{}' not found", name)));
                }
                if let Some((meta, preview)) = self.read_local_skill(&name).await? {
                    let mut result = json!({
                        "name": meta.name,
                        "description": meta.description,
                        "triggers": meta.triggers,
                        "tags": meta.tags,
                        "content_preview": preview
                    });
                    if let Some(entry) = manifest.skills.get(&name) {
                        result["source"] = json!(&entry.source);
                        result["origin_hash"] = json!(&entry.origin_hash);
                        result["installed_at"] = json!(entry.installed_at);
                    }
                    return Ok(result.to_string());
                }
                Err(ToolError::Execution(format!("failed to read skill '{}'", name)))
            }
            SkillAction::Remove { name } => {
                let manifest = self.read_manifest().await?;
                if !manifest.skills.contains_key(&name) && !self.skills_dir.join(&name).join(SKILL_FILE).exists() {
                    return Err(ToolError::Execution(format!("skill '{}' not found", name)));
                }
                let skill_dir = self.skills_dir.join(&name);
                if skill_dir.exists() {
                    tokio::fs::remove_dir_all(&skill_dir).await
                        .map_err(|e| ToolError::Execution(format!("failed to remove skill dir: {}", e)))?;
                }
                let mut manifest = manifest;
                manifest.skills.remove(&name);
                self.write_manifest(&manifest).await?;
                Ok(json!({ "status": "ok", "name": name }).to_string())
            }
            SkillAction::Search { query, limit } => {
                let limit = limit.unwrap_or(10);
                let results = self.search_remote(&query, limit).await?;
                Ok(json!({ "results": results }).to_string())
            }
            SkillAction::Sync => {
                // 读取 manifest 中所有已安装的 skills，从远程验证 hash
                let manifest = self.read_manifest().await?;
                let mut synced = 0;
                for (_name, entry) in manifest.skills.iter() {
                    if let Ok(resp) = self.http_client.get(&entry.source).send().await {
                        if resp.status().is_success() {
                            synced += 1;
                        }
                    }
                }
                Ok(json!({ "status": "ok", "synced_count": synced }).to_string())
            }
            SkillAction::Install { name, source } => {
                // source 可选：如果 manifest 中已有则复用，否则报错
                let manifest = self.read_manifest().await?;
                let source = source.or_else(|| manifest.skills.get(&name).map(|e| e.source.clone()));
                let source = source.ok_or_else(|| ToolError::Execution("source required for new install".to_string()))?;
                self.install_skill(&name, &source).await?;
                Ok(json!({ "status": "ok", "name": name, "installed_path": self.skills_dir.join(&name).to_string_lossy() }).to_string())
            }
        }
    }
}
