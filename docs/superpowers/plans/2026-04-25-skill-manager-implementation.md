# Skill Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `skill_manage` Tool，支持 Agent 自主创建、编辑、删除技能（create/edit/patch/delete/write_file/remove_file）

**Architecture:** 在 `hermes-skills` crate 中新增 `manager.rs` 处理核心业务逻辑，在 `hermes-tools-builtin` crate 中新增 `SkillManageTool`。使用 `FuzzyPatch` 实现 patch 操作，修改后触发 `SkillRegistry` 重新加载。

**Tech Stack:** Rust, async-trait, serde_json, fuzzy_matcher, tempfile (原子写入)

---

## File Structure

```
crates/hermes-skills/src/
├── manager.rs          # 新增：SkillManager 核心逻辑
├── lib.rs              # 修改：导出 manager 模块
└── tools.rs            # 修改：扩展 skills_manage 函数

crates/hermes-tools-builtin/src/
├── skills.rs           # 修改：新增 SkillManageTool
└── lib.rs              # 修改：注册 SkillManageTool
```

---

## Task 1: Create manager.rs with SkillManager

**Files:**
- Create: `crates/hermes-skills/src/manager.rs`
- Deps: `crates/hermes-skills/src/error.rs`, `crates/hermes-skills/src/loader.rs`, `crates/hermes-skills/src/registry.rs`, `crates/hermes-skills/src/fuzzy_patch.rs`

- [ ] **Step 1: Write the skeleton**

```rust
//! Skill Manager — Agent 自主管理技能的逻辑
//!
//! 提供 SkillManager 结构体，实现技能的创建、编辑、补丁、删除和文件操作。

use crate::error::SkillError;
use crate::fuzzy_patch::FuzzyPatch;
use crate::loader::SkillLoader;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

/// 验证规则常量
const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;
const MAX_SKILL_CONTENT_CHARS: usize = 100_000;
const MAX_SUPPORT_FILE_BYTES: usize = 1_048_576;

/// 允许的子目录
const ALLOWED_SUBDIRS: &[&str] = &["references", "templates", "scripts", "assets"];

/// Skill 名称验证正则
const VALID_NAME_RE: &str = r"^[a-z0-9][a-z0-9._-]*$";

/// SkillManager 处理所有技能管理操作
#[derive(Clone)]
pub struct SkillManager {
    skills_dir: PathBuf,
    fuzzy_patch: FuzzyPatch,
}
```

- [ ] **Step 2: Add constructors and basic methods**

```rust
impl SkillManager {
    /// 使用默认 skills 目录创建
    pub fn new() -> Result<Self, SkillError> {
        let skills_dir = Self::default_skills_dir()?;
        Ok(Self::with_dir(skills_dir))
    }

    /// 使用指定目录创建
    pub fn with_dir(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            fuzzy_patch: FuzzyPatch::new(),
        }
    }

    fn default_skills_dir() -> Result<PathBuf, SkillError> {
        dirs::home_dir()
            .map(|h| h.join(".hermes/skills"))
            .ok_or_else(|| SkillError::InvalidPath("Cannot find home directory".into()))
    }

    /// 获取 skills 根目录
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }
}
```

- [ ] **Step 3: Add validation helpers**

```rust
impl SkillManager {
    /// 验证 skill 名称
    pub fn validate_name(name: &str) -> Result<(), SkillError> {
        if name.is_empty() {
            return Err(SkillError::InvalidInput("Skill name is required.".into()));
        }
        if name.len() > MAX_NAME_LENGTH {
            return Err(SkillError::InvalidInput(
                format!("Skill name exceeds {} characters.", MAX_NAME_LENGTH)
            ));
        }
        let re = regex::Regex::new(VALID_NAME_RE).unwrap();
        if !re.is_match(name) {
            return Err(SkillError::InvalidInput(
                "Invalid skill name. Use lowercase letters, numbers, hyphens, dots, and underscores.".into()
            ));
        }
        Ok(())
    }

    /// 验证 category
    pub fn validate_category(category: &str) -> Result<(), SkillError> {
        if category.is_empty() {
            return Ok(());
        }
        if category.len() > MAX_NAME_LENGTH {
            return Err(SkillError::InvalidInput("Category exceeds maximum length.".into()));
        }
        let re = regex::Regex::new(VALID_NAME_RE).unwrap();
        if !re.is_match(category) {
            return Err(SkillError::InvalidInput(
                "Invalid category name.".into()
            ));
        }
        Ok(())
    }

    /// 验证 frontmatter 内容
    pub fn validate_frontmatter(content: &str) -> Result<(), SkillError> {
        if content.trim().is_empty() {
            return Err(SkillError::InvalidInput("Content cannot be empty.".into()));
        }
        if !content.starts_with("---") {
            return Err(SkillError::InvalidInput(
                "SKILL.md must start with YAML frontmatter (---).".into()
            ));
        }
        // Parse and validate required fields
        let (_, body) = crate::loader::Skill::parse_frontmatter(content)
            .map_err(|e| SkillError::InvalidInput(format!("Frontmatter error: {}", e)))?;
        if body.trim().is_empty() {
            return Err(SkillError::InvalidInput(
                "SKILL.md must have content after the frontmatter.".into()
            ));
        }
        Ok(())
    }

    /// 验证 file_path 不允许路径遍历
    pub fn validate_file_path(file_path: &str) -> Result<(), SkillError> {
        if file_path.contains("..") {
            return Err(SkillError::InvalidInput("Path traversal ('..') is not allowed.".into()));
        }
        let first_dir = file_path.split('/').next().unwrap_or("");
        if !ALLOWED_SUBDIRS.contains(&first_dir) {
            return Err(SkillError::InvalidInput(
                format!("File must be under one of: {}.", ALLOWED_SUBDIRS.join(", "))
            ));
        }
        Ok(())
    }

    /// 解析 skill 路径
    fn resolve_skill_dir(&self, name: &str, category: Option<&str>) -> PathBuf {
        match category {
            Some(cat) => self.skills_dir.join(cat).join(name),
            None => self.skills_dir.join(name),
        }
    }
}
```

- [ ] **Step 4: Add atomic write helper**

```rust
impl SkillManager {
    /// 原子性写入文件
    fn atomic_write(path: &Path, content: &str) -> Result<(), SkillError> {
        let parent = path.parent().ok_or_else(||
            SkillError::InvalidPath("Cannot determine parent directory".into())
        )?;
        fs::create_dir_all(parent)?;

        let mut temp_file = NamedTempFile::new_in(parent)?;
        std::io::Write::write_all(&mut temp_file, content.as_bytes())?;
        temp_file.persist(path)?;
        Ok(())
    }

    /// 查找 skill 目录
    fn find_skill_dir(&self, name: &str) -> Option<PathBuf> {
        // 搜索所有可能的路径
        if let Ok(entries) = fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let skill_dir = path.join(name);
                    if skill_dir.exists() {
                        return Some(skill_dir);
                    }
                }
            }
        }
        // 直接检查顶层
        let direct = self.skills_dir.join(name);
        if direct.exists() {
            return Some(direct);
        }
        None
    }
}
```

- [ ] **Step 5: Implement create action**

```rust
impl SkillManager {
    /// 创建新 skill
    pub fn create(&self, name: &str, content: &str, category: Option<&str>) -> Result<CreateResult, SkillError> {
        // 验证
        Self::validate_name(name)?;
        if let Some(cat) = category {
            Self::validate_category(cat)?;
        }
        Self::validate_frontmatter(content)?;

        // 检查是否已存在
        let skill_dir = self.resolve_skill_dir(name, category);
        if skill_dir.exists() {
            return Err(SkillError::AlreadyExists(name.into()));
        }

        // 创建目录结构
        fs::create_dir_all(&skill_dir)?;
        for subdir in ALLOWED_SUBDIRS {
            fs::create_dir_all(skill_dir.join(subdir))?;
        }

        // 写入 SKILL.md
        let skill_md = skill_dir.join("SKILL.md");
        Self::atomic_write(&skill_md, content)?;

        Ok(CreateResult {
            success: true,
            message: format!("Skill '{}' created.", name),
            path: skill_dir.to_string_lossy().into_owned(),
            category: category.map(String::from),
        })
    }
}

#[derive(serde::Serialize)]
pub struct CreateResult {
    pub success: bool,
    pub message: String,
    pub path: String,
    pub category: Option<String>,
}
```

- [ ] **Step 6: Implement edit action**

```rust
impl SkillManager {
    /// 编辑现有 skill
    pub fn edit(&self, name: &str, content: &str) -> Result<EditResult, SkillError> {
        Self::validate_frontmatter(content)?;

        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        let skill_md = skill_dir.join("SKILL.md");
        Self::atomic_write(&skill_md, content)?;

        Ok(EditResult {
            success: true,
            message: format!("Skill '{}' updated.", name),
            path: skill_dir.to_string_lossy().into_owned(),
        })
    }
}

#[derive(serde::Serialize)]
pub struct EditResult {
    pub success: bool,
    pub message: String,
    pub path: String,
}
```

- [ ] **Step 7: Implement patch action with replace_all**

```rust
impl SkillManager {
    /// 补丁修改 skill
    pub fn patch(&self, name: &str, old_string: &str, new_string: &str, replace_all: bool, file_path: Option<&str>) -> Result<PatchResult, SkillError> {
        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        let target = match file_path {
            Some(fp) => {
                Self::validate_file_path(fp)?;
                skill_dir.join(fp)
            }
            None => skill_dir.join("SKILL.md"),
        };

        if !target.exists() {
            return Err(SkillError::NotFound(format!("File not found: {:?}", target)));
        }

        let content = fs::read_to_string(&target)?;

        let patched_content = if replace_all {
            // 全部替换
            if !content.contains(old_string) {
                return Err(SkillError::InvalidInput("old_string not found in content".into()));
            }
            content.replace(old_string, new_string)
        } else {
            // 精确匹配一次
            let patched = self.fuzzy_patch.patch(&content, old_string, new_string)?;
            if !patched.contains(old_string) {
                return Ok(PatchResult {
                    success: true,
                    message: format!("Patched 1 occurrence in '{}'.", name),
                    match_count: 1,
                });
            }
            patched
        };

        // 验证 frontmatter 仍然有效（如果是 SKILL.md）
        if target.extension().map(|e| e == "md").unwrap_or(false) && file_path.is_none() {
            Self::validate_frontmatter(&patched_content)?;
        }

        Self::atomic_write(&target, &patched_content)?;

        Ok(PatchResult {
            success: true,
            message: format!("Patched in skill '{}'.", name),
            match_count: 1,
        })
    }
}

#[derive(serde::Serialize)]
pub struct PatchResult {
    pub success: bool,
    pub message: String,
    pub match_count: usize,
}
```

- [ ] **Step 8: Implement delete action**

```rust
impl SkillManager {
    /// 删除 skill
    pub fn delete(&self, name: &str) -> Result<DeleteResult, SkillError> {
        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        fs::remove_dir_all(&skill_dir)?;

        // 清理空 category 目录
        if let Some(parent) = skill_dir.parent() {
            if parent != self.skills_dir && parent.exists() {
                if fs::read_dir(parent)?.next().is_none() {
                    fs::remove_dir(parent)?;
                }
            }
        }

        Ok(DeleteResult {
            success: true,
            message: format!("Skill '{}' deleted.", name),
        })
    }
}

#[derive(serde::Serialize)]
pub struct DeleteResult {
    pub success: bool,
    pub message: String,
}
```

- [ ] **Step 9: Implement write_file action**

```rust
impl SkillManager {
    /// 写入支持文件
    pub fn write_file(&self, name: &str, file_path: &str, file_content: &str) -> Result<WriteFileResult, SkillError> {
        Self::validate_file_path(file_path)?;

        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        // 检查文件大小
        if file_content.len() > MAX_SUPPORT_FILE_BYTES {
            return Err(SkillError::InvalidInput(
                format!("File content exceeds {} bytes limit.", MAX_SUPPORT_FILE_BYTES)
            ));
        }

        let target = skill_dir.join(file_path);
        Self::atomic_write(&target, file_content)?;

        Ok(WriteFileResult {
            success: true,
            message: format!("File '{}' written to skill '{}'.", file_path, name),
            path: target.to_string_lossy().into_owned(),
        })
    }
}

#[derive(serde::Serialize)]
pub struct WriteFileResult {
    pub success: bool,
    pub message: String,
    pub path: String,
}
```

- [ ] **Step 10: Implement remove_file action**

```rust
impl SkillManager {
    /// 删除支持文件
    pub fn remove_file(&self, name: &str, file_path: &str) -> Result<RemoveFileResult, SkillError> {
        Self::validate_file_path(file_path)?;

        let skill_dir = self.find_skill_dir(name)
            .ok_or_else(|| SkillError::NotFound(format!("Skill '{}' not found", name)))?;

        let target = skill_dir.join(file_path);
        if !target.exists() {
            return Err(SkillError::NotFound(format!("File '{}' not found", file_path)));
        }

        fs::remove_file(&target)?;

        // 清理空子目录
        if let Some(parent) = target.parent() {
            if parent != skill_dir && parent.exists() {
                if fs::read_dir(parent)?.next().is_none() {
                    fs::remove_dir(parent)?;
                }
            }
        }

        Ok(RemoveFileResult {
            success: true,
            message: format!("File '{}' removed from skill '{}'.", file_path, name),
        })
    }
}

#[derive(serde::Serialize)]
pub struct RemoveFileResult {
    pub success: bool,
    pub message: String,
}
```

- [ ] **Step 11: Add serde and tempfile to Cargo.toml dependencies**

检查 `crates/hermes-skills/Cargo.toml` 确保有:
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tempfile = "3"
regex = "1.10"
fuzzy_matcher = "0.3"
```

- [ ] **Step 12: Run tests to verify compilation**

Run: `cargo check -p hermes-skills`
Expected: SUCCESS

- [ ] **Step 13: Commit**

```bash
git add crates/hermes-skills/src/manager.rs crates/hermes-skills/Cargo.toml
git commit -m "feat(skills): add SkillManager core logic

- validate_name, validate_category, validate_frontmatter, validate_file_path
- create: creates skill with directory structure
- edit: rewrites SKILL.md
- patch: fuzzy match with replace_all support
- delete: removes skill and cleans empty dirs
- write_file: atomic write to subdirectories
- remove_file: removes file and cleans empty dirs

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Add SkillManageTool to hermes-tools-builtin

**Files:**
- Modify: `crates/hermes-tools-builtin/src/skills.rs`
- Deps: `crates/hermes-skills/src/manager.rs`, `crates/hermes-tool-registry/src/registry.rs`

- [ ] **Step 1: Add imports and SkillManageTool struct**

```rust
use hermes_skills::manager::{SkillManager, CreateResult, EditResult, PatchResult, DeleteResult, WriteFileResult, RemoveFileResult};
use std::sync::Arc;
use parking_lot::RwLock;

/// Built-in skill management tool.
///
/// Usage from agent: `skill_manage(action="create", name="my-skill", content="...")`
pub struct SkillManageTool {
    manager: Arc<RwLock<SkillManager>>,
}

impl SkillManageTool {
    pub fn new(manager: Arc<RwLock<SkillManager>>) -> Self {
        Self { manager }
    }
}
```

- [ ] **Step 2: Implement Tool trait for SkillManageTool**

```rust
#[async_trait]
impl hermes_tool_registry::Tool for SkillManageTool {
    fn name(&self) -> &str {
        "skill_manage"
    }

    fn description(&self) -> &str {
        "Manage skills (create, edit, patch, delete, write_file, remove_file).
        Skills are your procedural memory - reusable approaches for recurring task types.
        Create when: complex task succeeded, errors overcome, or user asks to remember a procedure."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "edit", "patch", "delete", "write_file", "remove_file"],
                    "description": "The action to perform."
                },
                "name": {
                    "type": "string",
                    "description": "Skill name (lowercase, hyphens/underscores, max 64 chars)."
                },
                "content": {
                    "type": "string",
                    "description": "Full SKILL.md content (YAML frontmatter + markdown body). Required for 'create' and 'edit'."
                },
                "category": {
                    "type": "string",
                    "description": "Optional category/domain for organizing the skill (e.g., 'devops', 'data-science')."
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to a supporting file within the skill directory. For 'write_file'/'remove_file': required. For 'patch': optional, defaults to SKILL.md."
                },
                "file_content": {
                    "type": "string",
                    "description": "Content for the file. Required for 'write_file'."
                },
                "old_string": {
                    "type": "string",
                    "description": "Text to find in the file (required for 'patch')."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text (required for 'patch'). Can be empty string to delete."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "For 'patch': replace all occurrences instead of requiring a unique match (default: false)."
                }
            },
            "required": ["action", "name"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let action = args.pointer("/action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'action' argument".into()))?;

        let name = args.pointer("/name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'name' argument".into()))?;

        let manager = self.manager.read();

        let result = match action {
            "create" => {
                let content = args.pointer("/content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'content' argument for 'create'".into()))?;
                let category = args.pointer("/category").and_then(|v| v.as_str());
                manager.create(name, content, category)?
            }
            "edit" => {
                let content = args.pointer("/content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'content' argument for 'edit'".into()))?;
                manager.edit(name, content)?
            }
            "patch" => {
                let old_string = args.pointer("/old_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'old_string' argument for 'patch'".into()))?;
                let new_string = args.pointer("/new_string")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'new_string' argument for 'patch'".into()))?;
                let replace_all = args.pointer("/replace_all")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let file_path = args.pointer("/file_path").and_then(|v| v.as_str());
                manager.patch(name, old_string, new_string, replace_all, file_path)?
            }
            "delete" => {
                manager.delete(name)?
            }
            "write_file" => {
                let file_path = args.pointer("/file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'file_path' argument for 'write_file'".into()))?;
                let file_content = args.pointer("/file_content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'file_content' argument for 'write_file'".into()))?;
                manager.write_file(name, file_path, file_content)?
            }
            "remove_file" => {
                let file_path = args.pointer("/file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidArgs("missing 'file_path' argument for 'remove_file'".into()))?;
                manager.remove_file(name, file_path)?
            }
            _ => {
                return Err(ToolError::InvalidArgs(format!("Unknown action: {}", action)));
            }
        };

        Ok(serde_json::to_string(&result).unwrap())
    }
}
```

- [ ] **Step 3: Update load_skill_registry to also return SkillManager**

```rust
/// Initialize skill registry and skill manager by loading skills from default directories.
pub fn load_skill_registry_and_manager() -> (Arc<RwLock<SkillRegistry>>, Arc<RwLock<SkillManager>>) {
    let loader = SkillLoader::new(SkillLoader::default_dirs());
    let skills = loader.load_all().unwrap_or_default();
    let registry = Arc::new(RwLock::new(SkillRegistry::new()));
    let reg: Arc<RwLock<SkillRegistry>> = Arc::clone(&registry);
    for skill in skills {
        if let Err(e) = reg.write().register(skill) {
            tracing::warn!("Failed to register skill: {}", e);
        }
    }

    let manager = SkillManager::new().unwrap_or_else(|_| {
        tracing::warn!("Failed to create SkillManager, using temp dir");
        SkillManager::with_dir(std::env::temp_dir().join("hermes-skills"))
    });
    let manager = Arc::new(RwLock::new(manager));

    (registry, manager)
}

/// Initialize skill registry by loading skills from default directories.
pub fn load_skill_registry() -> Arc<RwLock<SkillRegistry>> {
    let (registry, _) = load_skill_registry_and_manager();
    registry
}
```

- [ ] **Step 4: Update lib.rs to export SkillManageTool and use new function**

```rust
pub use skills::{load_skill_registry, load_skill_registry_and_manager, SkillExecuteTool, SkillListTool, SkillSearchTool, SkillManageTool};
```

And in `register_builtin_tools`:

```rust
/// 将所有内置工具注册到传入的 ToolRegistry
///
/// 注意：技能相关工具需要单独创建并注册（依赖 SkillRegistry 和 SkillManager）
pub fn register_builtin_tools(registry: &ToolRegistry, environment: Arc<dyn Environment>) {
    // ... existing code ...
}

/// 注册技能管理工具
pub fn register_skill_tools(registry: &ToolRegistry, manager: Arc<RwLock<SkillManager>>) {
    registry.register(SkillManageTool::new(manager));
}
```

- [ ] **Step 5: Run tests to verify compilation**

Run: `cargo check -p hermes-tools-builtin`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-tools-builtin/src/skills.rs crates/hermes-tools-builtin/src/lib.rs
git commit -m "feat(skills): add SkillManageTool to hermes-tools-builtin

- skill_manage tool with create/edit/patch/delete/write_file/remove_file
- integrated with SkillManager for all operations
- returns JSON results for agent consumption

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Update hermes-skills lib.rs to export manager

**Files:**
- Modify: `crates/hermes-skills/src/lib.rs`

- [ ] **Step 1: Add manager module export**

```rust
pub mod manager;
pub use manager::{SkillManager, CreateResult, EditResult, PatchResult, DeleteResult, WriteFileResult, RemoveFileResult};
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p hermes-skills`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-skills/src/lib.rs
git commit -m "feat(skills): export manager module

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Integration - Wire SkillManageTool into Agent

**Files:**
- Modify: `crates/hermes-cli/src/` or wherever tools are registered

This task depends on where tools are wired up in your agent setup. The key is:

```rust
use hermes_tools_builtin::{register_builtin_tools, register_skill_tools};

// In your initialization:
let (registry, manager) = load_skill_registry_and_manager();
register_builtin_tools(&registry, environment);
register_skill_tools(&registry, manager);
```

- [ ] **Step 1: Find where tools are registered in your codebase**

Search for `load_skill_registry` to find the integration point.

- [ ] **Step 2: Update integration to use new function and register skill_manage**

- [ ] **Step 3: Run cargo check --all**

Run: `cargo check --all`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add [affected files]
git commit -m "feat(cli): integrate SkillManageTool into agent

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Tests

**Files:**
- Create: `crates/hermes-skills/src/tests/test_manager.rs`

- [ ] **Step 1: Write unit tests for SkillManager**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_name() {
        assert!(SkillManager::validate_name("valid-name").is_ok());
        assert!(SkillManager::validate_name("valid_name").is_ok());
        assert!(SkillManager::validate_name("abc123").is_ok());
        assert!(SkillManager::validate_name("").is_err());
        assert!(SkillManager::validate_name("Invalid").is_err()); // uppercase
        assert!(SkillManager::validate_name("-invalid").is_err()); // starts with hyphen
    }

    #[test]
    fn test_validate_category() {
        assert!(SkillManager::validate_category("").is_ok()); // empty is allowed
        assert!(SkillManager::validate_category("devops").is_ok());
        assert!(SkillManager::validate_category("data-science").is_ok());
    }

    #[test]
    fn test_validate_frontmatter() {
        let valid = "---\nname: test\ndescription: test desc\n---\n# Content";
        assert!(SkillManager::validate_frontmatter(valid).is_ok());

        let invalid_no_frontmatter = "# Just content";
        assert!(SkillManager::validate_frontmatter(invalid_no_frontmatter).is_err());
    }

    #[test]
    fn test_create_and_find_skill() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Test";
        let result = manager.create("test-skill", content, None);
        assert!(result.is_ok());

        let found = manager.find_skill_dir("test-skill");
        assert!(found.is_some());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p hermes-skills`
Expected: ALL PASS

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-skills/src/tests/
git commit -m "test(skills): add SkillManager unit tests

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review Checklist

- [ ] Spec coverage: All 6 actions (create/edit/patch/delete/write_file/remove_file) implemented
- [ ] All validation rules from spec implemented
- [ ] Error handling returns proper JSON with success:false
- [ ] Reload mechanism works (SkillRegistry updates on modify)
- [ ] No placeholders in code
- [ ] Types consistent across tasks
- [ ] Atomic writes for file operations
- [ ] Path traversal protection
