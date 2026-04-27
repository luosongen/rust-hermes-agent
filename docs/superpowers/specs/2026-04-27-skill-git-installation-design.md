# Skill Git 安装功能设计

> **Goal:** 实现从 git 仓库安装 skill 的功能

> **Architecture:** 使用 git2 crate 浅克隆仓库，walkdir 扫描 .md 文件，解析 frontmatter 后安装

> **Tech Stack:** Rust, git2, walkdir, serde_yaml

---

## 1. 概述

### 1.1 当前状态

`install_from_git` 函数（`crates/hermes-skills/src/hub/installer.rs:92-112`）尚未实现，只是返回错误：
```rust
return Err(HubError::InstallFailed(
    "Git installation not yet implemented".to_string(),
));
```

### 1.2 目标

实现从 git 仓库安装 skill 的完整功能：
- 支持任意 git URL（GitHub, GitLab 等）
- 自动扫描仓库中的所有 `.md` 文件
- 解析 frontmatter 提取 skill 元数据
- 安全扫描后再安装

---

## 2. 架构设计

### 2.1 安装流程

```
install_from_git(git_url, category, name, branch)
    │
    ├─► 1. git clone --depth 1 <git_url> -b <branch> <temp_dir>
    │
    ├─► 2. walkdir: find all **/*.md files
    │
    ├─► 3. For each .md file:
    │       ├─► Parse frontmatter (name, description)
    │       ├─► Security scan
    │       ├─► Copy to skills_dir/{category}/
    │       └─► Create SkillIndexEntry
    │
    └─► 4. Update index with all entries
```

### 2.2 Frontmatter 格式

每个 .md 文件可选包含 frontmatter：

```yaml
---
name: my-skill
description: This is my skill
category: custom
version: 1.0.0
---
```

- `name`: skill 名称（默认为文件名）
- `description`: skill 描述（默认为空）
- `category`: category 覆盖（默认使用传入的 category 参数）
- `version`: 版本号（默认为 "1.0.0"）

### 2.3 依赖

新增依赖：
```toml
git2 = "0.18"  # Git 仓库操作
```

已有依赖：
- `walkdir` - 目录遍历
- `serde_yaml` - frontmatter 解析
- `sha2` - checksum 计算

---

## 3. 实现细节

### 3.1 git clone

使用 `git2::Repository::clone` 进行浅克隆：

```rust
fn git_clone(url: &str, branch: &str, dest: &Path) -> Result<(), HubError> {
    let mut opts = git2::FetchOptions::new();
    opts.depth(1);  // 浅克隆

    let branch_ref = format!("refs/heads/{}", branch);
    let mut callbacks = git2::RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _cred_type| {
        git2::Cred::default()
    });

    opts.remote_callbacks(callbacks);

    git2::Repository::clone(url, dest, &mut opts)
        .map_err(|e| HubError::InstallFailed(e.to_string()))?;
}
```

### 3.2 扫描 .md 文件

```rust
fn find_skill_files(dir: &Path) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .glob("**/*.md")
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_path_buf())
        .collect()
}
```

### 3.3 Frontmatter 解析

```rust
fn parse_frontmatter(content: &str) -> (Metadata, &str) {
    if content.starts_with("---") {
        if let Some(end) = content.find("---").and_then(|p| content[3..].find("---")) {
            let yaml_str = &content[3..end];
            let body = &content[end + 6..];
            let meta: Metadata = serde_yaml::from_str(yaml_str).unwrap_or_default();
            return (meta, body.trim());
        }
    }
    (Metadata::default(), content.trim())
}
```

### 3.4 错误处理

| 场景 | 错误类型 |
|------|----------|
| git clone 失败 | `HubError::InstallFailed` |
| 无效的 git URL | `HubError::InstallFailed` |
| 安全扫描失败 | `HubError::SecurityBlocked` |
| 0 个 .md 文件 | `HubError::InstallFailed("No skill files found")` |

---

## 4. 文件变更

| 文件 | 变更 |
|------|------|
| `crates/hermes-skills/Cargo.toml` | 添加 `git2 = "0.18"` |
| `crates/hermes-skills/src/hub/installer.rs` | 实现 `install_from_git` |

---

## 5. 成功标准

1. `install_from_git` 可以从 GitHub 仓库安装 skill
2. 正确解析 .md 文件的 frontmatter
3. 安全扫描功能正常工作
4. 多个 .md 文件可以批量安装
5. 向后兼容：`install_from_market` 仍然正常工作
