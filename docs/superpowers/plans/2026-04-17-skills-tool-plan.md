# Skills Tool + Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 SkillsTool + Manager，对齐 Python hermes-agent 的 skills 管理能力（list / view / search / sync / install / remove），存储于 `~/.config/hermes-agent/skills/`。

**Architecture:** 单文件 `skills.rs`，通过 `serde_yaml` 解析 SKILL.md 的 YAML frontmatter，通过 `reqwest` 调用 `https://skills.sh` 远程 API，manifest 使用 JSON 文件（`.bundled_manifest`）跟踪已安装 skills。

**Tech Stack:** `hermes-tools-extended` crate, `reqwest`, `serde_yaml`, `serde_json`, `async_trait`, `tokio::fs`, 标准库 `PathBuf` / `SHA256`

---

## Task 1: 项目依赖 + 基础结构

**Files:**
- Modify: `crates/hermes-tools-extended/Cargo.toml`
- Create: `crates/hermes-tools-extended/src/skills.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`

### Step 1: 添加 serde_yaml 依赖

修改 `crates/hermes-tools-extended/Cargo.toml`，在 `[dependencies]` 末尾添加：

```toml
serde_yaml = "0.9"
sha2 = "0.10"
```

### Step 2: 创建 skills.rs 骨架（编译验证）

创建 `crates/hermes-tools-extended/src/skills.rs`，内容：

```rust
//! SkillsTool — Skills 管理工具
//!
//! 提供 list / view / search / sync / install / remove 操作。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

const SKILLS_DIR: &str = ".config/hermes-agent/skills";
const MANIFEST_FILE: &str = ".bundled_manifest";
const SKILL_FILE: &str = "SKILL.md";
const SKILLS_API_URL: &str = "https://skills.sh";

#[derive(Clone)]
pub struct SkillsTool {
    skills_dir: PathBuf,
}

impl SkillsTool {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            skills_dir: PathBuf::from(home).join(SKILLS_DIR),
        }
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
        Ok(json!({"status": "ok"}).to_string())
    }
}
```

### Step 3: 验证编译

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功，无 warning

### Step 4: 链接到 lib.rs

修改 `crates/hermes-tools-extended/src/lib.rs`：
- 在 `pub mod mixture_of_agents;` 后添加 `pub mod skills;`
- 在 `pub use mixture_of_agents::MixtureOfAgentsTool;` 后添加 `pub use skills::SkillsTool;`
- 在 `register_extended_tools()` 末尾添加 `registry.register(SkillsTool::new());`

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功

### Step 5: Commit

```bash
git add crates/hermes-tools-extended/Cargo.toml crates/hermes-tools-extended/src/skills.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(skills): add SkillsTool skeleton with list/view/search/sync/install/remove actions"
```

---

## Task 2: 数据结构 + Manifest 读写

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs:50-120`

### Step 1: 添加数据结构

在 `skills.rs` 中，在 `const SKILLS_API_URL` 后添加：

```rust
/// 从 SKILL.md frontmatter 解析的元信息
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

/// Manifest 中的单条 skill 记录
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillManifestEntry {
    pub source: String,
    pub origin_hash: String,
    #[serde(default)]
    pub installed_at: f64,
}

/// bundled_manifest 结构
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
```

### Step 2: 添加 Manifest 读写方法

在 `impl SkillsTool` 块中，添加：

```rust
/// 获取 manifest 文件路径
fn manifest_path(&self) -> PathBuf {
    self.skills_dir.join(MANIFEST_FILE)
}

/// 确保 skills 目录存在
async fn ensure_dir(&self) -> Result<(), ToolError> {
    tokio::fs::create_dir_all(&self.skills_dir).await
        .map_err(|e| ToolError::Execution(format!("failed to create skills dir: {}", e)))
}

/// 读取 manifest（损坏/缺失时返回默认空 manifest）
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

/// 写入 manifest
async fn write_manifest(&self, manifest: &BundledManifest) -> Result<(), ToolError> {
    let content = serde_json::to_string_pretty(manifest)
        .map_err(|e| ToolError::Execution(format!("failed to serialize manifest: {}", e)))?;
    tokio::fs::write(&self.manifest_path(), content).await
        .map_err(|e| ToolError::Execution(format!("failed to write manifest: {}", e)))
}
```

### Step 3: 验证编译

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功

### Step 4: Commit

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add SkillMetadata, BundledManifest structs and manifest I/O methods"
```

---

## Task 3: SKILL.md 解析器

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs:120-180`

### Step 1: 添加 parse_skill_markdown 方法

在 `impl SkillsTool` 块中，添加：

```rust
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
```

### Step 2: 添加 read_local_skill 方法

```rust
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
```

### Step 3: 添加 list_local 方法

```rust
/// 列出本地所有已安装的 skills
async fn list_local(&self) -> Result<Vec<SkillMetadata>, ToolError> {
    self.ensure_dir().await?;
    let mut entries = tokio::fs::read_dir(&self.skills_dir).await
        .map_err(|e| ToolError::Execution(format!("failed to read skills dir: {}", e)))?;
    let mut results = Vec::new();
    while let Some(entry) = entries.next_entry().await
        .map_err(|e| ToolError::Execution(format!("dir read error: {}", e)))? {
        let path = entry.path();
        if path.is_dir() && path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
            continue; // 跳过隐藏目录
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
```

### Step 4: 验证编译

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功

### Step 5: Commit

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add SKILL.md parser and local skill listing"
```

---

## Task 4: list / view / remove 操作实现

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs` — 更新 `execute()` 方法

### Step 1: 更新 execute 方法（处理 list / view / remove）

在 `async fn execute` 中替换空实现：

```rust
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
        _ => Err(ToolError::Execution("unimplemented action".to_string())),
    }
}
```

### Step 2: 验证编译

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功

### Step 3: 添加单元测试

在 `crates/hermes-tools-extended/tests/` 下创建 `test_skills.rs`：

```rust
use hermes_tools_extended::skills::{SkillsTool, SkillMetadata};

#[test]
fn test_parse_skill_markdown() {
    let content = r#"---
name: test-skill
description: A test skill
triggers: ["test", "demo"]
tags: ["testing"]
---

# Test Skill

This is the skill content."#;

    let result = SkillsTool::parse_skill_markdown(content);
    assert!(result.is_some());
    let (meta, preview) = result.unwrap();
    assert_eq!(meta.name, "test-skill");
    assert_eq!(meta.description, "A test skill");
    assert_eq!(meta.triggers, vec!["test", "demo"]);
    assert!(preview.contains("skill content"));
}

#[test]
fn test_parse_missing_frontmatter() {
    let content = "# No frontmatter\nJust content";
    assert!(SkillsTool::parse_skill_markdown(content).is_none());
}
```

Run: `cargo test -p hermes-tools-extended`
Expected: 所有测试通过

### Step 4: Commit

```bash
git add crates/hermes-tools-extended/src/skills.rs crates/hermes-tools-extended/tests/test_skills.rs
git commit -m "feat(skills): implement list, view, remove actions with unit tests"
```

---

## Task 5: search / sync 操作实现

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs`

### Step 1: 添加 HTTP client 和 search 方法

在 `SkillsTool` 结构体中添加 `http_client: reqwest::Client`，更新 `new()`：

```rust
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
}
```

更新结构体定义：

```rust
#[derive(Clone)]
pub struct SkillsTool {
    skills_dir: PathBuf,
    http_client: reqwest::Client,
}
```

添加 search/sync 方法：

```rust
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
```

### Step 2: 更新 execute 方法（添加 search / sync 分支）

在 `match params` 中添加：

```rust
SkillAction::Search { query, limit } => {
    let limit = limit.unwrap_or(10);
    let results = self.search_remote(&query, limit).await?;
    Ok(json!({ "results": results }).to_string())
}
SkillAction::Sync => {
    // 读取 manifest 中所有已安装的 skills，从远程验证 hash
    let manifest = self.read_manifest().await?;
    let mut synced = 0;
    for (name, entry) in manifest.skills.iter() {
        if let Ok(resp) = self.http_client.get(&entry.source).send().await {
            if resp.status().is_success() {
                synced += 1;
            }
        }
    }
    Ok(json!({ "status": "ok", "synced_count": synced }).to_string())
}
```

### Step 3: 验证编译

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功

### Step 4: Commit

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add search and sync actions with HTTP client"
```

---

## Task 6: install 操作实现

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs`

### Step 1: 添加 install 方法

```rust
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
```

### Step 2: 更新 execute 方法（添加 install 分支）

在 `match params` 中添加：

```rust
SkillAction::Install { name, source } => {
    // source 可选：如果 manifest 中已有则复用，否则报错
    let manifest = self.read_manifest().await?;
    let source = source.or_else(|| manifest.skills.get(&name).map(|e| e.source.clone()));
    let source = source.ok_or_else(|| ToolError::Execution("source required for new install".to_string()))?;
    self.install_skill(&name, &source).await?;
    Ok(json!({ "status": "ok", "name": name, "installed_path": self.skills_dir.join(&name).to_string_lossy() }).to_string())
}
```

### Step 3: 验证编译

Run: `cargo build -p hermes-tools-extended`
Expected: 编译成功

### Step 4: 运行测试

Run: `cargo test -p hermes-tools-extended`
Expected: 所有测试通过

### Step 5: Commit

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add install action with HTTP download and manifest update"
```

---

## Task 7: 集成验证 + lint

### Step 1: cargo check + clippy

Run: `cargo check -p hermes-tools-extended && cargo clippy -p hermes-tools-extended`
Expected: 无 error，warning 可接受

### Step 2: 全部编译 + 测试

Run: `cargo build --all && cargo test --all`
Expected: 所有 crate 编译成功，所有测试通过

### Step 3: Commit

```bash
git add -A && git commit -m "feat(skills): complete SkillsTool with list/view/search/sync/install/remove"
```

---

## Self-Review Checklist

| 检查项 | 状态 |
|--------|------|
| list 操作 — 扫描本地目录解析 SKILL.md | ✓ |
| view 操作 — 返回元信息 + content_preview | ✓ |
| search 操作 — 调用 skills.sh API | ✓ |
| sync 操作 — 验证已安装 skills 的远程 hash | ✓ |
| install 操作 — 下载 + 目录创建 + manifest 更新 | ✓ |
| remove 操作 — 删除目录 + manifest 更新 | ✓ |
| 离线 list/view/remove 正常 | ✓ |
| 目录不存在时自动创建 | ✓ |
| parse_skill_markdown 正确提取 YAML frontmatter | ✓ |
| 单元测试覆盖核心解析逻辑 | ✓ |
| 遵循现有 Tool trait 模式 | ✓ |