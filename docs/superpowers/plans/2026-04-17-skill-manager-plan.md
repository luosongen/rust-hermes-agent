# Skill Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend SkillsTool with create/edit/delete/patch/scan actions and security scanning module

**Architecture:** Extend existing SkillsTool with new actions, create standalone security_scanner.rs module

**Tech Stack:** Rust, regex for pattern matching, serde_yaml for frontmatter

---

## File Structure

```
crates/hermes-tools-extended/src/skills/
├── mod.rs              # Existing skills.rs (renamed from skills.rs)
├── security_scanner.rs # NEW: Security scanning module
```

```
crates/hermes-tools-extended/src/skills.rs  # EXISTING - modify to add new actions
```

---

## Task 1: Create security_scanner.rs module

**Files:**
- Create: `crates/hermes-tools-extended/src/skills/security_scanner.rs`

- [ ] **Step 1: Create security_scanner.rs with Threat and ScanResult types**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threat {
    pub pattern: String,
    pub line_number: usize,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub safe: bool,
    pub threats: Vec<Threat>,
}
```

- [ ] **Step 2: Add MALICIOUS_PATTERNS constant**

```rust
const MALICIOUS_PATTERNS: &[(&str, &str, Severity)] = &[
    (r"eval\s*\(", "eval() code execution", Severity::High),
    (r"exec\s*\(", "exec() code execution", Severity::High),
    (r"compile\s*\(", "compile() code generation", Severity::High),
    (r"subprocess", "subprocess command execution", Severity::High),
    (r"os\.system", "os.system shell execution", Severity::High),
    (r"os\.popen", "os.popen shell execution", Severity::High),
    (r"__import__", "__import__ dynamic import", Severity::High),
    (r"importlib", "importlib dynamic import", Severity::High),
    (r"open\s*=\s*", "open function override", Severity::Medium),
    (r"_builtin_\.open", "builtin open override", Severity::Medium),
    (r"os\.environ\[", "environment variable access", Severity::Medium),
    (r"getenv\s*\(", "environment variable read", Severity::Medium),
    (r"\|\s*sh", "shell pipe", Severity::High),
    (r"/bin/sh", "shell execution", Severity::High),
];
```

- [ ] **Step 3: Implement scan_content function**

```rust
pub fn scan_content(content: &str) -> ScanResult {
    let mut threats = Vec::new();
    for (line_number, line) in content.lines().enumerate() {
        for (pattern, description, severity) in MALICIOUS_PATTERNS {
            if line.contains(pattern) {
                threats.push(Threat {
                    pattern: description.to_string(),
                    line_number: line_number + 1,
                    severity: severity.clone(),
                });
            }
        }
    }
    ScanResult {
        safe: threats.is_empty(),
        threats,
    }
}
```

- [ ] **Step 4: Add module to mod.rs**

```rust
pub mod security_scanner;
pub use security_scanner::{scan_content, ScanResult, Threat, Severity};
```

- [ ] **Step 5: Write test for security scanner**

```rust
#[test]
fn test_scan_detects_eval() {
    let content = "let x = eval('2 + 2');";
    let result = scan_content(content);
    assert!(!result.safe);
    assert_eq!(result.threats.len(), 1);
    assert_eq!(result.threats[0].severity, Severity::High);
}

#[test]
fn test_scan_safe_content() {
    let content = "# This is a safe skill\nprint('hello')";
    let result = scan_content(content);
    assert!(result.safe);
    assert!(result.threats.is_empty());
}

#[test]
fn test_scan_multiple_threats() {
    let content = "eval('x')\nsubprocess.call(['ls'])";
    let result = scan_content(content);
    assert!(!result.safe);
    assert_eq!(result.threats.len(), 2);
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p hermes-tools-extended test_scan`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/hermes-tools-extended/src/skills/security_scanner.rs
git commit -m "feat(skills): add security scanner module for threat detection"
```

---

## Task 2: Add created_at/updated_at to SkillMetadata

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs:18-29`

- [ ] **Step 1: Update SkillMetadata struct**

```rust
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
    #[serde(default)]
    pub created_at: f64,
    #[serde(default)]
    pub updated_at: f64,
}
```

- [ ] **Step 2: Run tests to verify compilation**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add created_at and updated_at fields to SkillMetadata"
```

---

## Task 3: Add Create action

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs:244-329` (execute method)

- [ ] **Step 1: Add Create variant to SkillAction enum**

```rust
enum SkillAction {
    List,
    View { name: String },
    Search { query: String, #[serde(default)] limit: Option<usize> },
    Sync,
    Install { name: String, #[serde(default)] source: Option<String> },
    Remove { name: String },
    Create { name: String, content: String, #[serde(default)] triggers: Option<Vec<String>>, #[serde(default)] tags: Option<Vec<String>> },
    Edit { name: String, field: String, value: String },
    Delete { name: String },
    Patch { name: String, patch_content: String },
    Scan { #[serde(default)] name: Option<String> },
}
```

- [ ] **Step 2: Add create_skill method**

```rust
async fn create_skill(&self, name: &str, content: &str, triggers: Option<Vec<String>>, tags: Option<Vec<String>>) -> Result<(), ToolError> {
    self.ensure_dir().await?;

    let skill_dir = self.skills_dir.join(name);
    if skill_dir.join(SKILL_FILE).exists() {
        return Err(ToolError::Execution(format!("skill '{}' already exists", name)));
    }

    // Security scan
    let scan_result = crate::skills::security_scanner::scan_content(content);
    if !scan_result.safe {
        let threats: Vec<String> = scan_result.threats.iter()
            .map(|t| format!("{} at line {}", t.pattern, t.line_number))
            .collect();
        return Err(ToolError::Execution(format!("security scan failed: {}", threats.join("; "))));
    }

    // Generate frontmatter
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as f64;

    let triggers = triggers.unwrap_or_default();
    let tags = tags.unwrap_or_default();

    let frontmatter = format!(r#"---
name: {}
description: ""
triggers: {:?}
tags: {:?}
created_at: {}
updated_at: {}
---

{}"#, name, triggers, tags, now, now, content);

    tokio::fs::create_dir_all(&skill_dir).await
        .map_err(|e| ToolError::Execution(format!("failed to create dir: {}", e)))?;
    tokio::fs::write(skill_dir.join(SKILL_FILE), &frontmatter).await
        .map_err(|e| ToolError::Execution(format!("failed to write skill file: {}", e)))?;

    Ok(())
}
```

- [ ] **Step 3: Handle Create in execute method**

```rust
SkillAction::Create { name, content, triggers, tags } => {
    self.create_skill(&name, &content, triggers, tags).await?;
    Ok(json!({ "status": "ok", "name": name }).to_string())
}
```

- [ ] **Step 4: Run build and tests**

Run: `cargo build -p hermes-tools-extended && cargo test -p hermes-tools-extended test_parse_skill_markdown`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add create action with security scanning"
```

---

## Task 4: Add Edit and Delete actions

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs`

- [ ] **Step 1: Add edit_skill method**

```rust
async fn edit_skill(&self, name: &str, field: &str, value: &str) -> Result<(), ToolError> {
    let skill_path = self.skills_dir.join(name).join(SKILL_FILE);
    if !skill_path.exists() {
        return Err(ToolError::Execution(format!("skill '{}' not found", name)));
    }

    let content = tokio::fs::read_to_string(&skill_path).await
        .map_err(|e| ToolError::Execution(format!("failed to read skill: {}", e)))?;

    let (mut meta, body) = Self::parse_skill_markdown(&content)
        .ok_or_else(|| ToolError::Execution("failed to parse skill frontmatter".to_string()))?;

    // Update field
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as f64;
    meta.updated_at = now;

    match field {
        "description" => meta.description = value.to_string(),
        "triggers" => {
            meta.triggers = serde_json::from_str(value)
                .map_err(|e| ToolError::InvalidArgs(format!("invalid triggers JSON: {}", e)))?;
        }
        "tags" => {
            meta.tags = serde_json::from_str(value)
                .map_err(|e| ToolError::InvalidArgs(format!("invalid tags JSON: {}", e)))?;
        }
        _ => return Err(ToolError::InvalidArgs(format!("unknown field: {}", field))),
    }

    // Reconstruct file
    let frontmatter = format!(r#"---
name: {}
description: "{}"
triggers: {:?}
tags: {:?}
created_at: {}
updated_at: {}
---

{}"#, meta.name, meta.description, meta.triggers, meta.tags, meta.created_at, meta.updated_at, body.trim());

    tokio::fs::write(&skill_path, &frontmatter).await
        .map_err(|e| ToolError::Execution(format!("failed to write skill: {}", e)))?;

    Ok(())
}
```

- [ ] **Step 2: Handle Edit in execute method**

```rust
SkillAction::Edit { name, field, value } => {
    self.edit_skill(&name, &field, &value).await?;
    Ok(json!({ "status": "ok", "name": name }).to_string())
}
```

- [ ] **Step 3: Add delete_skill method (for local delete, keep Remove for manifest removal)**

```rust
async fn delete_skill(&self, name: &str) -> Result<(), ToolError> {
    let skill_dir = self.skills_dir.join(name);
    if !skill_dir.exists() {
        return Err(ToolError::Execution(format!("skill '{}' not found", name)));
    }

    tokio::fs::remove_dir_all(&skill_dir).await
        .map_err(|e| ToolError::Execution(format!("failed to delete skill: {}", e)))?;

    Ok(())
}
```

- [ ] **Step 4: Handle Delete in execute method**

```rust
SkillAction::Delete { name } => {
    self.delete_skill(&name).await?;
    Ok(json!({ "status": "ok", "name": name }).to_string())
}
```

- [ ] **Step 5: Run build and tests**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add edit and delete actions"
```

---

## Task 5: Add Patch and Scan actions

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs`

- [ ] **Step 1: Add patch_skill method**

```rust
async fn patch_skill(&self, name: &str, patch_content: &str) -> Result<(), ToolError> {
    let skill_path = self.skills_dir.join(name).join(SKILL_FILE);
    if !skill_path.exists() {
        return Err(ToolError::Execution(format!("skill '{}' not found", name)));
    }

    // Security scan the patch content
    let scan_result = crate::skills::security_scanner::scan_content(patch_content);
    if !scan_result.safe {
        let threats: Vec<String> = scan_result.threats.iter()
            .map(|t| format!("{} at line {}", t.pattern, t.line_number))
            .collect();
        return Err(ToolError::Execution(format!("security scan failed: {}", threats.join("; "))));
    }

    let content = tokio::fs::read_to_string(&skill_path).await
        .map_err(|e| ToolError::Execution(format!("failed to read skill: {}", e)))?;

    // Append patch to body
    let (meta, body) = Self::parse_skill_markdown(&content)
        .ok_or_else(|| ToolError::Execution("failed to parse skill frontmatter".to_string()))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as f64;

    let new_body = format!("{}\n\n---\n\n{}", body.trim(), patch_content);

    let frontmatter = format!(r#"---
name: {}
description: "{}"
triggers: {:?}
tags: {:?}
created_at: {}
updated_at: {}
---

{}"#, meta.name, meta.description, meta.triggers, meta.tags, meta.created_at, now, new_body);

    tokio::fs::write(&skill_path, &frontmatter).await
        .map_err(|e| ToolError::Execution(format!("failed to write skill: {}", e)))?;

    Ok(())
}
```

- [ ] **Step 2: Handle Patch in execute method**

```rust
SkillAction::Patch { name, patch_content } => {
    self.patch_skill(&name, &patch_content).await?;
    Ok(json!({ "status": "ok", "name": name }).to_string())
}
```

- [ ] **Step 3: Add scan_skills method**

```rust
async fn scan_skills(&self, name: Option<&str>) -> Result<ScanResult, ToolError> {
    let skills = if let Some(name) = name {
        vec![(name.to_string(), self.skills_dir.join(name).join(SKILL_FILE))]
    } else {
        // Scan all skills
        let mut paths = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.skills_dir).await
            .map_err(|e| ToolError::Execution(format!("failed to read dir: {}", e)))?;
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| ToolError::Execution(format!("dir read error: {}", e)))? {
            let path = entry.path();
            if path.is_dir() && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
                paths.push((path.file_name().unwrap().to_string_lossy().to_string(), path.join(SKILL_FILE)));
            }
        }
        paths
    };

    let mut all_threats = Vec::new();
    let mut scanned_count = 0;

    for (skill_name, skill_path) in skills {
        if skill_path.exists() {
            scanned_count += 1;
            if let Ok(content) = tokio::fs::read_to_string(&skill_path).await {
                // Extract body content (skip frontmatter)
                if let Some((_, body)) = Self::parse_skill_markdown(&content) {
                    let result = crate::skills::security_scanner::scan_content(&body);
                    if !result.safe {
                        for threat in result.threats {
                            all_threats.push(json!({
                                "skill": skill_name,
                                "pattern": threat.pattern,
                                "line_number": threat.line_number,
                                "severity": threat.severity
                            }));
                        }
                    }
                }
            }
        }
    }

    Ok(ScanResult {
        safe: all_threats.is_empty(),
        threats: all_threats,
        scanned_count,
    })
}
```

- [ ] **Step 4: Handle Scan in execute method**

```rust
SkillAction::Scan { name } => {
    let result = self.scan_skills(name.as_deref()).await?;
    Ok(json!({
        "scanned": result.scanned_count,
        "safe": result.safe,
        "threats_found": result.threats.len(),
        "results": result.threats
    }).to_string())
}
```

- [ ] **Step 5: Run build and tests**

Run: `cargo build -p hermes-tools-extended`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-tools-extended/src/skills.rs
git commit -m "feat(skills): add patch and scan actions"
```

---

## Task 6: Update parameters schema and lib.rs exports

**Files:**
- Modify: `crates/hermes-tools-extended/src/skills.rs` (parameters method)
- Modify: `crates/hermes-tools-extended/src/lib.rs`

- [ ] **Step 1: Update parameters() to include new actions**

```rust
fn parameters(&self) -> serde_json::Value {
    json!({
        "type": "object",
        "oneOf": [
            {"properties": {"action": {"const": "list"}}, "required": ["action"]},
            {"properties": {"action": {"const": "view"}, "name": {"type": "string"}}, "required": ["action", "name"]},
            {"properties": {"action": {"const": "search"}, "query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["action", "query"]},
            {"properties": {"action": {"const": "sync"}}, "required": ["action"]},
            {"properties": {"action": {"const": "install"}, "name": {"type": "string"}, "source": {"type": "string"}}, "required": ["action", "name"]},
            {"properties": {"action": {"const": "remove"}, "name": {"type": "string"}}, "required": ["action", "name"]},
            {"properties": {"action": {"const": "create"}, "name": {"type": "string"}, "content": {"type": "string"}, "triggers": {"type": "array"}, "tags": {"type": "array"}}, "required": ["action", "name", "content"]},
            {"properties": {"action": {"const": "edit"}, "name": {"type": "string"}, "field": {"type": "string"}, "value": {"type": "string"}}, "required": ["action", "name", "field", "value"]},
            {"properties": {"action": {"const": "delete"}, "name": {"type": "string"}}, "required": ["action", "name"]},
            {"properties": {"action": {"const": "patch"}, "name": {"type": "string"}, "patch_content": {"type": "string"}}, "required": ["action", "name", "patch_content"]},
            {"properties": {"action": {"const": "scan"}, "name": {"type": "string"}}, "required": ["action"]}
        ]
    })
}
```

- [ ] **Step 2: Update lib.rs exports if needed (should already export SkillsTool)**

Verify `pub use skills::SkillsTool;` exists in lib.rs

- [ ] **Step 3: Run full test suite**

Run: `cargo test -p hermes-tools-extended`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-extended/src/skills.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(skills): update parameters schema for all new actions"
```

---

## Task 7: Integration verification

**Files:**
- Create: `crates/hermes-tools-extended/tests/test_skill_manager.rs`

- [ ] **Step 1: Create integration test file**

```rust
use hermes_tools_extended::skills::SkillsTool;
use hermes_tools_extended::skills::security_scanner::{scan_content, Severity};

#[test]
fn test_security_scanner_rejects_eval() {
    let content = "dangerous = eval('malicious code')";
    let result = scan_content(content);
    assert!(!result.safe);
    assert!(result.threats.iter().any(|t| t.severity == Severity::High));
}

#[test]
fn test_security_scanner_rejects_subprocess() {
    let content = "import subprocess\nsubprocess.run(['ls'])";
    let result = scan_content(content);
    assert!(!result.safe);
}

#[test]
fn test_security_scanner_allows_safe_code() {
    let content = "# This is a helpful skill\nprint('Hello, World!')";
    let result = scan_content(content);
    assert!(result.safe);
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test -p hermes-tools-extended -- --nocapture`
Expected: ALL PASS

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p hermes-tools-extended`
Expected: No warnings

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat(skills): add skill manager integration tests"
```

---

## Self-Review Checklist

- [ ] Spec coverage: All requirements have corresponding tasks
- [ ] No placeholders: All code is complete
- [ ] Type consistency: SkillMetadata, ScanResult, Threat types match spec
- [ ] Tests: Each task includes test verification
