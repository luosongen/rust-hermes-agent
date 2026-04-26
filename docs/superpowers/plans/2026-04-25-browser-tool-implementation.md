# BrowserTool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 BrowserTool，通过 agent-browser CLI 调用本地 headless Chromium，支持页面导航、交互和内容提取

**Architecture:** BrowserTool 单例管理浏览器会话，通过 tokio::process::Command 调用 agent-browser CLI。Session 存储在 BrowserSessionStore (HashMap) 中，支持超时自动清理。

**Tech Stack:** tokio::process, uuid, JSON parsing

---

## File Structure

```
crates/hermes-tools-builtin/src/
└── browser_tools.rs      # BrowserTool + BrowserSessionStore + 所有工具

crates/hermes-tools-builtin/tests/
└── test_browser.rs       # 单元测试
```

---

## Task 1: Core types + session management

**Files:**
- Modify: `crates/hermes-tools-builtin/src/browser_tools.rs` (create new file)

- [ ] **Step 1: Write test for BrowserSessionStore**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_session_store_default() {
        let store = BrowserSessionStore::default();
        assert!(store.sessions.is_empty());
        assert!(store.task_session_map.is_empty());
    }

    #[test]
    fn test_create_and_get_session() {
        let mut store = BrowserSessionStore::default();
        let session = store.create_session("task-123");
        assert_eq!(session.task_id, "task-123");
        assert!(!session.session_name.is_empty());

        let retrieved = store.get_session("task-123");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_cleanup_stale_sessions() {
        let mut store = BrowserSessionStore::default();
        store.create_session("task-old");
        // Manually set last_activity to old time for testing
        // cleanup_stale() should remove it
        store.cleanup_stale(300);
        assert!(store.get_session("task-old").is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
error: cannot find type `BrowserSessionStore` in module
```

- [ ] **Step 3: Write BrowserSessionStore implementation**

```rust
//! BrowserTool - 浏览器自动化工具
//!
//! 通过 agent-browser CLI 调用本地 headless Chromium

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio::time::timeout;
use uuid::Uuid;

const INACTIVITY_TIMEOUT_SECS: u64 = 300;

/// Browser Session
#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub session_name: String,
    pub task_id: String,
    pub socket_dir: PathBuf,
    pub created_at: f64,
    pub last_activity: f64,
}

/// Browser Session Store
#[derive(Default)]
pub struct BrowserSessionStore {
    sessions: HashMap<String, BrowserSession>,
    task_session_map: HashMap<String, String>,
}

impl BrowserSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建新会话
    pub fn create_session(&mut self, task_id: &str) -> BrowserSession {
        let session_name = format!("h_{}", Uuid::new_v4().to_string()[..8].to_string());
        let socket_dir = std::env::temp_dir().join(format!("agent-browser-{}", session_name));

        let session = BrowserSession {
            session_name: session_name.clone(),
            task_id: task_id.to_string(),
            socket_dir: socket_dir.clone(),
            created_at: current_timestamp(),
            last_activity: current_timestamp(),
        };

        self.sessions.insert(session_name.clone(), session.clone());
        self.task_session_map.insert(task_id.to_string(), session_name);

        session
    }

    /// 获取会话
    pub fn get_session(&self, task_id: &str) -> Option<&BrowserSession> {
        let session_name = self.task_session_map.get(task_id)?;
        self.sessions.get(session_name)
    }

    /// 获取会话（可变）
    pub fn get_session_mut(&mut self, task_id: &str) -> Option<&mut BrowserSession> {
        let session_name = self.task_session_map.get(task_id)?;
        let session = self.sessions.get_mut(session_name)?;
        session.last_activity = current_timestamp();
        Some(session)
    }

    /// 清理过期会话
    pub fn cleanup_stale(&mut self, timeout_secs: u64) {
        let now = current_timestamp();
        let stale: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| now - s.last_activity > timeout_secs as f64)
            .map(|(name, _)| name.clone())
            .collect();

        for name in stale {
            if let Some(session) = self.sessions.remove(&name) {
                self.task_session_map.remove(&session.task_id);
            }
        }
    }
}

fn current_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
```

- [ ] **Step 4: Run test to verify it passes**

```
cargo test -p hermes-tools-builtin -- browser_tools --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-tools-builtin/src/browser_tools.rs
git commit -m "feat(browser): add BrowserTool core types and session management"
```

---

## Task 2: Run command wrapper

**Files:**
- Modify: `crates/hermes-tools-builtin/src/browser_tools.rs`

- [ ] **Step 1: Write test for run_command**

```rust
#[tokio::test]
async fn test_run_command_not_found() {
    let store = BrowserSessionStore::new();
    let tool = BrowserTool::new(store);

    // Should return error when agent-browser not found
    let result = tool.run_command("task-1", "version", &[]).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify behavior**

```
# If agent-browser not installed, should get error
```

- [ ] **Step 3: Write run_command implementation**

```rust
impl BrowserTool {
    /// 运行 agent-browser 命令
    pub async fn run_command(
        &self,
        task_id: &str,
        cmd: &str,
        args: &[&str],
    ) -> Result<serde_json::Value, ToolError> {
        let session = self
            .store
            .read()
            .await
            .get_session(task_id)
            .ok_or_else(|| ToolError::Execution("No browser session found. Call browser_navigate first.".into()))?;

        let mut command = Command::new("agent-browser");
        command
            .arg("--session")
            .arg(&session.session_name)
            .arg("--json")
            .arg(cmd)
            .args(args)
            .env("AGENT_BROWSER_SOCKET_DIR", &session.socket_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = timeout(Duration::from_secs(30), command.output())
            .await
            .map_err(|_| ToolError::Execution("Browser command timeout".into()))?
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!("agent-browser error: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&stdout)
            .map_err(|e| ToolError::Execution(format!("Failed to parse agent-browser output: {}", e)))
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-builtin/src/browser_tools.rs
git commit -m "feat(browser): add run_command wrapper for agent-browser CLI"
```

---

## Task 3: Basic browser tools (navigate, snapshot, click, type, scroll, back, press)

**Files:**
- Modify: `crates/hermes-tools-builtin/src/browser_tools.rs`

- [ ] **Step 1: Write BrowserTool struct and Tool impl**

```rust
/// BrowserTool
pub struct BrowserTool {
    store: Arc<RwLock<BrowserSessionStore>>,
}

impl BrowserTool {
    pub fn new(store: BrowserSessionStore) -> Self {
        Self {
            store: Arc::new(RwLock::new(store)),
        }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }

    fn description(&self) -> &str {
        "Navigate to a URL in the browser"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string" },
                "url": { "type": "string" }
            },
            "required": ["task_id", "url"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let task_id = args.pointer("/task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing task_id".into()))?;
        let url = args.pointer("/url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing url".into()))?;

        // Create session if not exists
        let session_name = {
            let mut store = self.store.write().await;
            if store.get_session(task_id).is_none() {
                store.create_session(task_id);
            }
            store.get_session(task_id).unwrap().session_name.clone()
        };

        let result = self.run_command(task_id, "open", &[url]).await?;
        Ok(serde_json::to_string(&result).unwrap_or_default())
    }
}
```

- [ ] **Step 2: Add all tool implementations**

For each tool (browser_snapshot, browser_click, browser_type, browser_scroll, browser_back, browser_press), implement the Tool trait.

- [ ] **Step 3: Register tools in register_builtin_tools**

Modify `crates/hermes-tools-builtin/src/lib.rs` to register BrowserTool instances.

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-tools-builtin/src/browser_tools.rs crates/hermes-tools-builtin/src/lib.rs
git commit -m "feat(browser): add basic browser tools (navigate, snapshot, click, type, scroll, back, press)"
```

---

## Task 4: browser_vision + session cleanup

**Files:**
- Modify: `crates/hermes-tools-builtin/src/browser_tools.rs`

- [ ] **Step 1: Write browser_vision tool**

```rust
/// browser_vision - 截图 + VisionTool 分析
async fn execute_vision(&self, args: serde_json::Value) -> Result<String, ToolError> {
    let task_id = args.pointer("/task_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs("missing task_id".into()))?;
    let question = args.pointer("/question")
        .and_then(|v| v.as_str())
        .unwrap_or("Describe what you see in this screenshot");
    let annotate = args.pointer("/annotate")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Run screenshot command
    let screenshot_path = format!("/tmp/hermes-screenshots/browser_{}.png", Uuid::new_v4().to_string()[..8]);
    let mut args_vec = vec!["--full", &screenshot_path];
    let result = self.run_command(task_id, "screenshot", &args_vec).await?;

    // TODO: Integrate with VisionTool for AI analysis

    Ok(serde_json::json!({
        "success": true,
        "screenshot_path": screenshot_path,
        "analysis": "Screenshot captured"
    }).to_string())
}
```

- [ ] **Step 2: Add session cleanup background task**

```rust
impl BrowserTool {
    pub fn start_cleanup_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let mut store = self.store.write().await;
                store.cleanup_stale(INACTIVITY_TIMEOUT_SECS);
            }
        });
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-tools-builtin/src/browser_tools.rs
git commit -m "feat(browser): add browser_vision and session cleanup task"
```

---

## Task 5: Tests

**Files:**
- Create: `crates/hermes-tools-builtin/tests/test_browser.rs`

- [ ] **Step 1: Write integration tests**

```rust
use hermes_tools_builtin::browser_tools::BrowserTool;

#[tokio::test]
async fn test_browser_tool_creation() {
    let tool = BrowserTool::new();
    assert_eq!(tool.name(), "browser_navigate");
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p hermes-tools-builtin --test test_browser
```

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-tools-builtin/tests/test_browser.rs
git commit -m "test(browser): add integration tests"
```

---

## Verification Checklist

- [ ] All 5 tasks complete
- [ ] Tests pass: `cargo test -p hermes-tools-builtin -- browser`
- [ ] Code compiles: `cargo build -p hermes-tools-builtin`
- [ ] No clippy warnings: `cargo clippy -p hermes-tools-builtin`
