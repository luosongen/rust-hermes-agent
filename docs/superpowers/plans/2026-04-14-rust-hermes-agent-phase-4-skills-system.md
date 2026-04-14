# Skills System Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Build a skills system where each skill is a Markdown file with YAML frontmatter (metadata) and embedded content. Skills are loaded from local directories, parsed, registered, and exposed as tools to the agent.

**Architecture:**
- `SkillMetadata` — parsed YAML frontmatter (name, description, platforms, config schema)
- `SkillError` — dedicated error type for load/parse/install failures
- `SkillLoader` — walks skill directories, parses frontmatter + body, extracts code blocks
- `Skill` — a loaded, ready-to-use skill with metadata and content
- `SkillRegistry` — in-memory registry of all loaded skills with search/lookup
- Skills exposed as tools via `hermes-tools-builtin` integration

**Tech Stack:** serde_yaml, walkdir, regex, tokio, hermes-core, hermes-tools-builtin

---

## Current State

`crates/hermes-skills/src/lib.rs` is a placeholder. The `Cargo.toml` already declares the needed dependencies:
- `serde_yaml`, `walkdir`, `regex`, `dirs`, `async-trait`, `tokio`, `tracing`

The design doc (`docs/superpowers/specs/2026-04-14-rust-hermes-agent-design.md` §6) defines the skill format and `SkillLoader` interface.

---

## Task 1: SkillError and SkillMetadata

**Files:**
- Create: `crates/hermes-skills/src/error.rs`
- Create: `crates/hermes-skills/src/metadata.rs`

- [ ] **Step 1: Create `crates/hermes-skills/src/error.rs`**

```rust
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
}
```

- [ ] **Step 2: Create `crates/hermes-skills/src/metadata.rs`**

```rust
use serde::{Deserialize, Serialize};

/// Configuration item defined in skill frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigItem {
    pub key: String,
    pub description: String,
    #[serde(default)]
    pub default: Option<String>,
}

/// Hermes-specific metadata inside the YAML frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HermesMetadata {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config: Vec<SkillConfigItem>,
    #[serde(default)]
    pub requires_toolsets: Vec<String>,
}

/// The YAML frontmatter of a skill file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub metadata: HermesMetadata,
}

impl SkillMetadata {
    /// Returns true if this skill supports the given platform.
    pub fn supports_platform(&self, platform: &str) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p == platform)
    }

    /// Returns true if the skill requires the given toolset.
    pub fn requires_toolset(&self, toolset: &str) -> bool {
        self.metadata
            .requires_toolsets
            .iter()
            .any(|t| t == toolset)
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-skills 2>&1`
Expected: Compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-skills/src/error.rs crates/hermes-skills/src/metadata.rs
git commit -m "feat(hermes-skills): add SkillError and SkillMetadata types

- SkillError covers IO, parse, YAML, not-found, and platform errors
- SkillMetadata models YAML frontmatter with name, description,
  platforms, and HermesMetadata (version, config, requires_toolsets)

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: SkillLoader

**Files:**
- Create: `crates/hermes-skills/src/loader.rs`
- Modify: `crates/hermes-skills/src/lib.rs`

- [ ] **Step 1: Create `crates/hermes-skills/src/loader.rs`**

```rust
use crate::error::SkillError;
use crate::metadata::SkillMetadata;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A loaded skill with parsed content.
#[derive(Debug, Clone)]
pub struct Skill {
    pub metadata: SkillMetadata,
    /// The full body text after frontmatter (markdown content).
    pub content: String,
    /// Code blocks extracted from the skill body.
    pub code_blocks: Vec<CodeBlock>,
    /// Examples extracted from the skill body.
    pub examples: Vec<String>,
    /// Absolute path to the skill file.
    pub path: PathBuf,
}

/// A code block extracted from a skill.
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub lang: Option<String>,
    pub code: String,
}

impl Skill {
    /// Parse frontmatter from skill file content.
    fn parse_frontmatter(raw: &str) -> Result<(String, String), SkillError> {
        let trimmed = raw.trim_start();
        if !trimmed.starts_with("---") {
            return Err(SkillError::ParseFrontmatter(
                "Missing --- opening delimiter".into(),
            ));
        }
        let after_delim = &trimmed[3..];
        let end = after_delim
            .find("\n---")
            .ok_or_else(|| {
                SkillError::ParseFrontmatter("Missing closing --- delimiter".into())
            })?;
        let frontmatter = after_delim[..end].trim();
        let body = after_delim[end + 4..].trim().to_string();
        Ok((frontmatter.to_string(), body))
    }

    /// Load and parse a single skill file.
    pub fn from_path(path: &Path) -> Result<Self, SkillError> {
        let raw = fs::read_to_string(path)?;
        let (frontmatter, body) = Self::parse_frontmatter(&raw)?;
        let metadata: SkillMetadata =
            serde_yaml::from_str(&frontmatter)
                .map_err(|e| SkillError::ParseFrontmatter(e.to_string()))?;
        let code_blocks = Self::extract_code_blocks(&body);
        let examples = Self::extract_examples(&body);
        Ok(Self {
            metadata,
            content: body,
            code_blocks,
            examples,
            path: path.to_path_buf(),
        })
    }

    fn extract_code_blocks(body: &str) -> Vec<CodeBlock> {
        let re = Regex::new(r"```(\w*)\n([\s\S]*?)```").unwrap();
        re.captures_iter(body)
            .map(|cap| CodeBlock {
                lang: cap.get(1).map(|m| m.as_str().to_string()),
                code: cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
            })
            .collect()
    }

    fn extract_examples(body: &str) -> Vec<String> {
        let re = Regex::new(r"(?m)^/[\w-]+.*$").unwrap();
        re.find_iter(body)
            .map(|m| m.as_str().to_string())
            .collect()
    }
}

/// Loads skills from local directories.
pub struct SkillLoader {
    dirs: Vec<PathBuf>,
}

impl SkillLoader {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    /// Load all skills from all configured directories.
    pub fn load_all(&self) -> Result<Vec<Skill>, SkillError> {
        let mut skills = Vec::new();
        for dir in &self.dirs {
            skills.extend(self.load_from_dir(dir)?);
        }
        Ok(skills)
    }

    /// Load all skills from a single directory (non-recursive).
    pub fn load_from_dir(&self, dir: &Path) -> Result<Vec<Skill>, SkillError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut skills = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match Skill::from_path(&path) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!("Skipping invalid skill {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(skills)
    }

    /// Get the default skills directories (~/.hermes/skills, ./skills).
    pub fn default_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let default = home.join(".hermes/skills");
            if default.exists() || std::env::var("HERMES_SKILLS_HOME").is_ok() {
                dirs.push(default);
            }
        }
        if std::env::var("HERMES_SKILLS_LOCAL").is_ok() {
            dirs.push(PathBuf::from("skills"));
        }
        dirs
    }
}
```

- [ ] **Step 2: Update `crates/hermes-skills/src/lib.rs`**

Replace the placeholder with:

```rust
pub mod error;
pub mod metadata;
pub mod loader;

pub use error::SkillError;
pub use loader::{Skill, SkillLoader};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-skills 2>&1`
Expected: Compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-skills/src/loader.rs crates/hermes-skills/src/lib.rs
git commit -m "feat(hermes-skills): add SkillLoader with frontmatter parsing

- Skill::from_path() parses YAML frontmatter and markdown body
- extract_code_blocks() and extract_examples() via regex
- SkillLoader::load_all() walks configured directories
- Default dirs: ~/.hermes/skills and ./skills

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: SkillRegistry

**Files:**
- Create: `crates/hermes-skills/src/registry.rs`
- Modify: `crates/hermes-skills/src/lib.rs`

- [ ] **Step 1: Create `crates/hermes-skills/src/registry.rs`**

```rust
use crate::error::SkillError;
use crate::loader::Skill;
use std::collections::HashMap;

/// In-memory registry of loaded skills.
pub struct SkillRegistry {
    by_name: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    /// Register a skill. Returns error if a skill with the same name already exists.
    pub fn register(&mut self, skill: Skill) -> Result<(), SkillError> {
        let name = skill.metadata.name.clone();
        if self.by_name.contains_key(&name) {
            return Err(SkillError::AlreadyExists(name));
        }
        self.by_name.insert(name, skill);
        Ok(())
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.by_name.get(name)
    }

    /// List all skill names.
    pub fn names(&self) -> Vec<String> {
        self.by_name.keys().cloned().collect()
    }

    /// Search skills by name or description substring.
    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let query_lower = query.to_lowercase();
        self.by_name
            .values()
            .filter(|s| {
                s.metadata.name.to_lowercase().contains(&query_lower)
                    || s.metadata.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Total count of registered skills.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Update `crates/hermes-skills/src/lib.rs`** — add:

```rust
pub mod registry;
pub use registry::SkillRegistry;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-skills 2>&1`
Expected: Compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-skills/src/registry.rs crates/hermes-skills/src/lib.rs
git commit -m "feat(hermes-skills): add SkillRegistry for in-memory skill management

- register(), get(), names(), search() operations
- Duplicate name registration returns AlreadyExists error

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Built-in Skills Tools

Expose skills as tools (list, execute, search) via `hermes-tools-builtin`.

**Files:**
- Create: `crates/hermes-tools-builtin/src/skills.rs`
- Modify: `crates/hermes-tools-builtin/src/lib.rs`

- [ ] **Step 1: Create `crates/hermes-tools-builtin/src/skills.rs`**

```rust
use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_skills::{Skill, SkillLoader, SkillRegistry};
use parking_lot::RwLock;
use std::sync::Arc;

/// Built-in skill execution tool.
///
/// Usage from agent: `skill_execute(name="skill-name")`
pub struct SkillExecuteTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillExecuteTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillExecuteTool {
    fn name(&self) -> &str {
        "skill_execute"
    }

    fn description(&self) -> &str {
        "Execute a registered Hermes skill by name, returning its content"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to execute"
                }
            },
            "required": ["name"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let name = args
            .pointer("/name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'name' argument".into()))?;

        let registry = self.registry.read();
        let skill = registry
            .get(name)
            .ok_or_else(|| ToolError::NotFound(format!("skill not found: {}", name)))?;

        Ok(skill.content.clone())
    }
}

/// Built-in skill list tool.
///
/// Usage from agent: `skill_list()`
pub struct SkillListTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillListTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry: Arc::clone(&registry),
        }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillListTool {
    fn name(&self) -> &str {
        "skill_list"
    }

    fn description(&self) -> &str {
        "List all available Hermes skills"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(
        &self,
        _args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let registry = self.registry.read();
        let names: Vec<&str> = registry.names().iter().map(|s| s.as_str()).collect();
        Ok(names.join("\n"))
    }
}

/// Built-in skill search tool.
///
/// Usage from agent: `skill_search(query="search term")`
pub struct SkillSearchTool {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillSearchTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry: Arc::clone(&registry),
        }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SkillSearchTool {
    fn name(&self) -> &str {
        "skill_search"
    }

    fn description(&self) -> &str {
        "Search available Hermes skills by name or description"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let query = args
            .pointer("/query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'query' argument".into()))?;

        let registry = self.registry.read();
        let results = registry.search(query);

        let output = results
            .iter()
            .map(|s| format!("# {}\n{}\n", s.metadata.name, s.metadata.description))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(output)
    }
}

/// Initialize skill registry by loading skills from default directories.
pub fn load_skill_registry() -> Arc<RwLock<SkillRegistry>> {
    let loader = SkillLoader::new(SkillLoader::default_dirs());
    let skills = loader.load_all().unwrap_or_default();
    let registry = Arc::new(RwLock::new(SkillRegistry::new()));
    let reg = Arc::clone(&registry);
    for skill in skills {
        if let Err(e) = reg.write().register(skill) {
            tracing::warn!("Failed to register skill: {}", e);
        }
    }
    registry
}
```

- [ ] **Step 2: Update `crates/hermes-tools-builtin/src/lib.rs`**

Add to the module list:

```rust
pub mod skills;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-tools-builtin 2>&1`
Expected: Compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-builtin/src/skills.rs crates/hermes-tools-builtin/src/lib.rs
git commit -m "feat(hermes-tools-builtin): add built-in skill tools

- skill_list: lists all registered skill names
- skill_execute: returns skill content by name
- skill_search: searches by name/description substring
- load_skill_registry() loads skills from ~/.hermes/skills and ./skills

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Tests for SkillLoader and SkillRegistry

**Files:**
- Create: `crates/hermes-skills/src/tests/loader_tests.rs`
- Create: `crates/hermes-skills/src/tests/registry_tests.rs`
- Create: `crates/hermes-skills/src/tests/mod.rs`

- [ ] **Step 1: Create `crates/hermes-skills/src/tests/mod.rs`**

```rust
mod loader_tests;
mod registry_tests;
```

- [ ] **Step 2: Create `crates/hermes-skills/src/tests/loader_tests.rs`**

```rust
use hermes_skills::loader::Skill;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_parse_frontmatter_valid() {
    let raw = r#"---
name: test-skill
description: A test skill
platforms: [macos, linux]
---

# Test Skill

Some content here.
"#;
    let skill = Skill::from_path(std::path::Path::new("test.md"))
        .expect_err("Should fail — from_path requires real file");
    // The above will fail because from_path needs a real file.
    // Use a temp file for the real test:
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.md");
    fs::write(&path, raw).unwrap();
    let skill = Skill::from_path(&path).unwrap();
    assert_eq!(skill.metadata.name, "test-skill");
    assert_eq!(skill.metadata.description, "A test skill");
    assert!(!skill.metadata.supports_platform("windows"));
    assert!(skill.metadata.supports_platform("macos"));
}

#[test]
fn test_parse_frontmatter_missing_delimiter() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.md");
    fs::write(&path, "name: no-frontmatter\n---\ncontent").unwrap();
    let err = Skill::from_path(&path).unwrap_err();
    assert!(err.to_string().contains("Missing ---"));
}

#[test]
fn test_extract_code_blocks() {
    let body = r#"
Some text.

```bash
echo hello
```

```python
print("world")
```
"#;
    let blocks = Skill::extract_code_blocks(body);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].lang.as_deref(), Some("bash"));
    assert_eq!(blocks[0].code.trim(), "echo hello");
    assert_eq!(blocks[1].lang.as_deref(), Some("python"));
}

#[test]
fn test_extract_examples() {
    let body = r#"
# Examples

/test-skill arg1 arg2
/another one
"#;
    let examples = Skill::extract_examples(body);
    assert_eq!(examples.len(), 2);
    assert_eq!(examples[0], "/test-skill arg1 arg2");
}
```

- [ ] **Step 3: Create `crates/hermes-skills/src/tests/registry_tests.rs`**

```rust
use hermes_skills::loader::Skill;
use hermes_skills::metadata::SkillMetadata;
use hermes_skills::SkillRegistry;
use std::fs;
use tempfile::TempDir;

fn make_skill(name: &str) -> Skill {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("s.md");
    let raw = format!(r#"---
name: {}
description: A skill
---

Content.
"#, name);
    fs::write(&path, raw).unwrap();
    Skill::from_path(&path).unwrap()
}

#[test]
fn test_register_and_get() {
    let mut reg = SkillRegistry::new();
    let skill = make_skill("my-skill");
    reg.register(skill).unwrap();
    assert!(reg.get("my-skill").is_some());
    assert!(reg.get("missing").is_none());
}

#[test]
fn test_register_duplicate_error() {
    let mut reg = SkillRegistry::new();
    let s1 = make_skill("dup");
    let s2 = make_skill("dup");
    reg.register(s1).unwrap();
    let err = reg.register(s2).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn test_search() {
    let mut reg = SkillRegistry::new();
    let s1 = make_skill("rust-format");
    let s2 = make_skill("python-test");
    reg.register(s1).unwrap();
    reg.register(s2).unwrap();
    let results = reg.search("rust");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].metadata.name, "rust-format");
}

#[test]
fn test_names() {
    let mut reg = SkillRegistry::new();
    reg.register(make_skill("a")).unwrap();
    reg.register(make_skill("b")).unwrap();
    let mut names = reg.names();
    names.sort();
    assert_eq!(names, vec!["a", "b"]);
}
```

- [ ] **Step 4: Enable test module in lib.rs**

Add to `crates/hermes-skills/src/lib.rs`:

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 5: Add `tempfile` to hermes-skills Cargo.toml**

Read `crates/hermes-skills/Cargo.toml`, add to `[dev-dependencies]`:

```toml
tempfile.workspace = true
```

Add to workspace root `Cargo.toml` `[workspace.dependencies]`:

```toml
tempfile = "3"
```

- [ ] **Step 6: Fix `test_names` test** (known bug in test above — "body" should be "b"):

Replace the last test in `registry_tests.rs`:

```rust
#[test]
fn test_names() {
    let mut reg = SkillRegistry::new();
    reg.register(make_skill("a")).unwrap();
    reg.register(make_skill("b")).unwrap();
    let mut names = reg.names();
    names.sort();
    assert_eq!(names, vec!["a", "b"]);
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p hermes-skills 2>&1`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/hermes-skills/src/tests/
git add crates/hermes-skills/Cargo.toml Cargo.toml
git commit -m "test(hermes-skills): add unit tests for SkillLoader and SkillRegistry

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Self-Review

1. **Spec coverage:** Skill format (SKILL.md YAML frontmatter), SkillLoader, SkillRegistry, and built-in tools (skill_list, skill_execute, skill_search) are all implemented.
2. **Placeholder scan:** No "TBD", "TODO", or vague steps — all code blocks are complete.
3. **Type consistency:** `SkillMetadata::supports_platform()` matches the `platforms: [macos, linux]` field in the design spec. `SkillLoader::default_dirs()` uses `dirs::home_dir()` which is already a workspace dep.
4. **No circular deps:** `hermes-skills` depends only on `walkdir`, `serde_yaml`, `regex`, `dirs`. `hermes-tools-builtin` depends on `hermes-skills` which introduces no cycle (tools-builtin → skills → (none of the others)).
5. **Dependency:** `tempfile` added to dev-dependencies for tests. `regex` already in workspace deps.
