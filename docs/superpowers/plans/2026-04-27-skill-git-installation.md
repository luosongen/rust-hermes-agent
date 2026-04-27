# Skill Git 安装功能实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现从 git 仓库安装 skill 的功能，使用 git2 浅克隆，walkdir 扫描 .md 文件，解析 frontmatter 后安装

**Architecture:** 在 `installer.rs` 中添加 `install_from_git` 函数，使用 git2 crate 进行浅克隆，walkdir 扫描 .md 文件，serde_yaml 解析 frontmatter

**Tech Stack:** Rust, git2, walkdir, serde_yaml, sha2

---

## 文件结构

```
crates/hermes-skills/
├── Cargo.toml                              # 添加 git2 依赖
└── src/hub/
    ├── installer.rs                       # 实现 install_from_git
    ├── types.rs                            # 添加 Metadata 结构体
    └── error.rs                           # 确保错误类型完整
```

---

## Task 1: 添加 git2 依赖

**Files:**
- Modify: `crates/hermes-skills/Cargo.toml`

- [ ] **Step 1: 添加 git2 依赖**

在 `[dependencies]` 部分添加：

```toml
git2 = "0.18"
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p hermes-skills 2>&1 | head -10`
Expected: 编译成功（只有警告）

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-skills/Cargo.toml
git commit -m "chore(skills): 添加 git2 依赖"
```

---

## Task 2: 添加 Metadata 结构体

**Files:**
- Modify: `crates/hermes-skills/src/hub/types.rs`

- [ ] **Step 1: 添加 Metadata 结构体**

在 `types.rs` 文件末尾添加：

```rust
/// Frontmatter 元数据
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub version: Option<String>,
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p hermes-skills 2>&1 | head -10`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-skills/src/hub/types.rs
git commit -m "feat(skills): 添加 Metadata 结构体用于 frontmatter 解析"
```

---

## Task 3: 实现辅助函数

**Files:**
- Modify: `crates/hermes-skills/src/hub/installer.rs`

- [ ] **Step 1: 添加辅助函数**

在 `installer.rs` 顶部（use 语句之后）添加：

```rust
/// 从 git URL 克隆仓库（浅克隆）
fn git_clone(url: &str, branch: &str, dest: &std::path::Path) -> Result<(), HubError> {
    let mut opts = git2::FetchOptions::new();
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_url, _username_from_url, _cred_type| {
        git2::Cred::default()
    });
    opts.remote_callbacks(callbacks);

    let reference = format!("refs/heads/{}", branch);
    git2::Repository::clone(url, dest, &mut opts)
        .map_err(|e| HubError::InstallFailed(format!("Git clone failed: {}", e)))?;
    Ok(())
}

/// 查找目录中所有 .md 文件
fn find_markdown_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(dir)
        .glob("**/*.md")
        .into_iter()
        .filter_map(|e| e.ok())
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
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p hermes-skills 2>&1 | head -15`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-skills/src/hub/installer.rs
git commit -m "feat(skills): 添加 git clone 和 frontmatter 解析辅助函数"
```

---

## Task 4: 实现 install_from_git 函数

**Files:**
- Modify: `crates/hermes-skills/src/hub/installer.rs`

- [ ] **Step 1: 替换 TODO 实现**

找到 `install_from_git` 函数（大约 line 92-112），将其替换为：

```rust
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
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p hermes-skills 2>&1 | head -20`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-skills/src/hub/installer.rs
git commit -m "feat(skills): 实现 install_from_git 函数"
```

---

## Task 5: 验证向后兼容

**Files:**
- Test: `crates/hermes-skills/tests/`

- [ ] **Step 1: 运行现有测试**

Run: `cargo test -p hermes-skills 2>&1 | tail -20`
Expected: 所有现有测试通过

- [ ] **Step 2: 验证编译**

Run: `cargo check --all 2>&1 | grep "^error" | head -5`
Expected: 无错误

- [ ] **Step 3: 提交最终变更**

```bash
git add -A
git commit -m "feat(skills): 完成 git 安装功能实现"
```

---

## 成功标准检查清单

- [ ] `install_from_git` 可以从 git URL 安装 skill
- [ ] 正确解析 .md 文件的 frontmatter
- [ ] 安全扫描功能正常工作
- [ ] 多个 .md 文件可以批量安装
- [ ] 向后兼容：`install_from_market` 仍然正常工作
