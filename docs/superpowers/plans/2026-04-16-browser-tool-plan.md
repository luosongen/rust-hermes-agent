# BrowserTool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现浏览器自动化工具 BrowserTool，通过 `agent-browser` CLI 调用本地 headless Chromium，支持页面导航、交互和内容提取。

**Architecture:**
- BrowserTool 单例管理浏览器会话生命周期，使用 `Arc<RwLock<BrowserSessionStore>>` 存储会话
- `BrowserSessionStore` 通过 `HashMap<session_name, BrowserSession>` 管理所有会话
- 每个 task_id 映射到对应 session_name，实现会话隔离
- 通过 `tokio::process::Command` 调用 `agent-browser --session <name> --json <cmd>`
- Session 5 分钟无活动自动清理（后台任务）
- browser_vision 依赖 `VisionTool`（位于 `hermes-tools-extended` crate）

**Tech Stack:** Rust async/await, tokio::process, async_trait, hermes-core, hermes-tool-registry, uuid

---

## 文件结构

```
crates/hermes-tools-builtin/src/
├── browser_tools.rs      # BrowserTool + BrowserSessionStore（新建）
└── lib.rs                # 模块导出 + register_builtin_tools 更新

crates/hermes-tools-builtin/tests/
└── test_browser.rs       # 单元测试（新建）
```

**依赖（需添加到 Cargo.toml）：**
- `uuid.workspace = true`（已有）
- `tokio` features: `process`, `time`, `sync`, `rt`（已有）

**跨 crate 依赖注意事项：**
- `VisionTool` 位于 `hermes-tools-extended` crate，`browser_vision` 需要调用它
- 需要在 `hermes-tools-builtin` 中添加 `hermes-tools-extended` 依赖，或在 `browser_vision` 失败时返回友好错误

---

## Task 1: BrowserSessionStore + 核心类型

**Files:**
- Create: `crates/hermes-tools-builtin/src/browser_tools.rs`
- Modify: `crates/hermes-tools-builtin/src/lib.rs`
- Modify: `crates/hermes-tools-builtin/Cargo.toml`
- Test: `crates/hermes-tools-builtin/tests/test_browser.rs`

### Step 1: 添加 uuid 依赖到 hermes-tools-builtin

检查 `Cargo.toml` 是否已有 `uuid = { workspace = true }`，没有则添加。

### Step 2: 创建 browser_tools.rs 框架

```rust
//! browser_tools — 浏览器自动化工具
//!
//! 通过 agent-browser CLI 调用本地 headless Chromium。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Session 超时时间（秒）
const INACTIVITY_TIMEOUT_SECS: u64 = 300;

/// 浏览器会话
#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub session_name: String,
    pub task_id: String,
    pub socket_dir: PathBuf,
    pub created_at: f64,
    pub last_activity: f64,
}

/// 会话存储
#[derive(Debug, Default)]
pub struct BrowserSessionStore {
    sessions: HashMap<String, BrowserSession>,
    task_session_map: HashMap<String, String>,
}

impl BrowserSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self, task_id: &str) -> BrowserSession {
        let session_name = format!("h_{}", Uuid::new_v4().to_string()[..10].to_string());
        let socket_dir = std::env::temp_dir()
            .join(format!("agent-browser-{}", session_name));
        let now = now();
        let session = BrowserSession {
            session_name: session_name.clone(),
            task_id: task_id.to_string(),
            socket_dir: socket_dir.clone(),
            created_at: now,
            last_activity: now,
        };
        self.sessions.insert(session_name.clone(), session.clone());
        self.task_session_map.insert(task_id.to_string(), session_name);
        session
    }

    pub fn get_session(&self, task_id: &str) -> Option<&BrowserSession> {
        let session_name = self.task_session_map.get(task_id)?;
        self.sessions.get(session_name)
    }

    pub fn get_session_mut(&mut self, task_id: &str) -> Option<&mut BrowserSession> {
        let session_name = self.task_session_map.get(task_id)?;
        let session = self.sessions.get_mut(session_name)?;
        Some(session)
    }

    pub fn touch(&mut self, task_id: &str) {
        if let Some(session) = self.get_session_mut(task_id) {
            session.last_activity = now();
        }
    }

    pub fn remove_session(&mut self, task_id: &str) {
        if let Some(session_name) = self.task_session_map.remove(task_id) {
            self.sessions.remove(&session_name);
        }
    }

    pub fn get_stale_sessions(&self) -> Vec<String> {
        let now = now();
        self.sessions
            .iter()
            .filter(|(_, s)| now - s.last_activity > INACTIVITY_TIMEOUT_SECS as f64)
            .map(|(_, s)| s.task_id.clone())
            .collect()
    }

    pub fn cleanup_stale(&mut self) {
        let stale = self.get_stale_sessions();
        for task_id in stale {
            self.remove_session(&task_id);
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// BrowserTool
pub struct BrowserTool {
    store: Arc<RwLock<BrowserSessionStore>>,
    config_dir: std::path::PathBuf,
}

impl BrowserTool {
    pub fn new(config_dir: std::path::PathBuf) -> Self {
        Self {
            store: Arc::new(RwLock::new(BrowserSessionStore::new())),
            config_dir,
        }
    }
}

impl Clone for BrowserTool {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            config_dir: self.config_dir.clone(),
        }
    }
}

impl std::fmt::Debug for BrowserTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserTool").finish()
    }
}
```

### Step 3: 实现 agent-browser 命令执行

在 `BrowserTool` 中添加 `run_command` 方法：

```rust
use std::process::Stdio;

#[derive(Debug, Deserialize)]
struct BrowserResponse {
    success: bool,
    #[serde(default)]
    data: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
}

impl BrowserTool {
    /// 执行 agent-browser 命令
    async fn run_command(
        &self,
        task_id: &str,
        subcmd: &str,
        args: Vec<&str>,
        timeout_secs: u64,
    ) -> Result<serde_json::Value, ToolError> {
        // 获取或创建会话
        let (session_name, socket_dir) = {
            let mut store = self.store.write();
            // 如果没有会话，返回错误（navigate 除外，它会创建会话）
            let session = store.get_session(task_id)
                .ok_or_else(|| ToolError::InvalidArgs("No active session. Call browser_navigate first.".into()))?;
            store.touch(task_id);
            (session.session_name.clone(), session.socket_dir.clone())
        };

        // 构建命令: agent-browser --session <name> --json <subcmd> [args...]
        let mut cmd = tokio::process::Command::new("agent-browser");
        cmd.arg("--session").arg(&session_name);
        cmd.arg("--json").arg(subcmd);
        cmd.args(&args);
        cmd.env("AGENT_BROWSER_SOCKET_DIR", &socket_dir);

        // 设置超时
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| ToolError::Execution("Command timed out".into()))?
        .map_err(|e| ToolError::Execution(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!("agent-browser failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let resp: BrowserResponse = serde_json::from_str(&stdout)
            .map_err(|e| ToolError::Execution(format!("Invalid JSON from agent-browser: {}", e)))?;

        if !resp.success {
            return Err(ToolError::Execution(
                resp.error.unwrap_or_else(|| "Unknown error".into())
            ));
        }

        Ok(resp.data)
    }
}
```

### Step 4: 写测试验证 BrowserSessionStore

```rust
// crates/hermes-tools-builtin/tests/test_browser.rs
use hermes_tools_builtin::browser_tools::{BrowserSessionStore, BrowserTool};

#[test]
fn test_create_session() {
    let mut store = BrowserSessionStore::new();
    let session = store.create_session("task_1");
    assert!(session.session_name.starts_with("h_"));
    assert_eq!(session.task_id, "task_1");
}

#[test]
fn test_get_session() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    let session = store.get_session("task_1").unwrap();
    assert_eq!(session.task_id, "task_1");
}

#[test]
fn test_remove_session() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    store.remove_session("task_1");
    assert!(store.get_session("task_1").is_none());
}

#[test]
fn test_touch_updates_last_activity() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    let before = store.get_session("task_1").unwrap().last_activity;
    std::thread::sleep(std::time::Duration::from_millis(10));
    store.touch("task_1");
    let after = store.get_session("task_1").unwrap().last_activity;
    assert!(after > before);
}

#[test]
fn test_cleanup_stale() {
    let mut store = BrowserSessionStore::new();
    store.create_session("task_1");
    // Manually set to old time
    {
        let s = store.sessions.get_mut("h_").unwrap();
        s.last_activity = 0.0;
    }
    store.cleanup_stale();
    assert!(store.get_session("task_1").is_none());
}
```

### Step 5: 运行测试验证

Run: `cargo test -p hermes-tools-builtin test_browser -- --nocapture 2>&1 | head -30`
Expected: PASS（部分测试可能在 store 内部方法上需要调整）

---

## Task 2: 基础 Browser 工具实现

**Files:**
- Modify: `crates/hermes-tools-builtin/src/browser_tools.rs`

### Step 1: 添加 Tool 实现框架

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateParams {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotParams {
    #[serde(default)]
    pub full: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickParams {
    pub r#ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeParams {
    pub r#ref: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollParams {
    pub direction: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PressParams {
    pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionParams {
    pub question: String,
    #[serde(default)]
    pub annotate: bool,
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }

    fn description(&self) -> &str {
        "Navigate to a URL in the browser."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: NavigateParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // 创建或获取会话
        let (session_name, socket_dir) = {
            let mut store = self.store.write();
            let session = store.get_session_mut(&context.session_id)
                .cloned()
                .unwrap_or_else(|| store.create_session(&context.session_id));
            store.touch(&context.session_id);
            (session.session_name, session.socket_dir)
        };

        // 执行 open 命令
        let output = self.run_browser_open(&session_name, &socket_dir, &params.url).await?;

        Ok(json!({
            "success": true,
            "url": output.get("url").and_then(|v| v.as_str()).unwrap_or(&params.url),
            "title": output.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            "snapshot": output.get("snapshot").and_then(|v| v.as_str()).unwrap_or(""),
            "element_count": output.get("element_count").and_then(|v| v.as_u64()).unwrap_or(0)
        }).to_string())
    }
}
```

### Step 2: 实现 browser_navigate

```rust
impl BrowserTool {
    async fn run_browser_open(
        &self,
        session_name: &str,
        socket_dir: &PathBuf,
        url: &str,
    ) -> Result<serde_json::Value, ToolError> {
        let mut cmd = tokio::process::Command::new("agent-browser");
        cmd.arg("--session").arg(session_name);
        cmd.arg("--json").arg("open");
        cmd.arg(url);
        cmd.env("AGENT_BROWSER_SOCKET_DIR", socket_dir);

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            cmd.output(),
        )
        .await
        .map_err(|_| ToolError::Execution("Navigation timed out".into()))?
        .map_err(|e| ToolError::Execution(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!("open failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let resp: BrowserResponse = serde_json::from_str(&stdout)
            .map_err(|e| ToolError::Execution(format!("Invalid JSON: {}", e)))?;

        if !resp.success {
            return Err(ToolError::Execution(
                resp.error.unwrap_or_else(|| "open failed".into())
            ));
        }

        Ok(resp.data)
    }
}
```

### Step 3: 实现 browser_snapshot

```rust
// 在 Tool impl 中添加
fn name(&self) -> &str { "browser_snapshot" }
fn description(&self) -> &str { "Get text-based accessibility tree snapshot." }
fn parameters(&self) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "full": { "type": "boolean", "default": false }
        }
    })
}

async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
    let params: SnapshotParams = serde_json::from_value(args)
        .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

    let mut args_vec = vec![];
    if !params.full {
        args_vec.push("-c".to_string()); // compact mode
    }

    let data = self.run_command(&context.session_id, "snapshot", args_vec, 30).await?;

    Ok(json!({
        "success": true,
        "snapshot": data.get("snapshot").and_then(|v| v.as_str()).unwrap_or(""),
        "element_count": data.get("refs").and_then(|v| v.as_array()).map(|a| a.len() as u64).unwrap_or(0)
    }).to_string())
}
```

### Step 4: 实现 browser_click, browser_type, browser_scroll, browser_back, browser_press

每个工具添加单独的 `match` 分支，使用共享的 `run_command` 方法。

```rust
async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
    match self.name() {
        "browser_navigate" => { /* ... */ }
        "browser_snapshot" => { /* ... */ }
        "browser_click" => {
            let params: ClickParams = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let mut ref_str = params.ref;
            if !ref_str.starts_with('@') {
                ref_str = format!("@{}", ref_str);
            }
            self.run_command(&context.session_id, "click", vec![&ref_str], 30).await?;
            Ok(json!({ "success": true, "clicked": ref_str }).to_string())
        }
        "browser_type" => {
            let params: TypeParams = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let mut ref_str = params.ref;
            if !ref_str.starts_with('@') {
                ref_str = format!("@{}", ref_str);
            }
            self.run_command(&context.session_id, "fill", vec![&ref_str, &params.text], 30).await?;
            Ok(json!({ "success": true, "typed": params.text, "element": ref_str }).to_string())
        }
        "browser_scroll" => {
            let params: ScrollParams = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            let dir = &params.direction;
            if dir != "up" && dir != "down" {
                return Err(ToolError::InvalidArgs("direction must be 'up' or 'down'".into()));
            }
            self.run_command(&context.session_id, "scroll", vec![dir, "500"], 30).await?;
            Ok(json!({ "success": true, "scrolled": dir }).to_string())
        }
        "browser_back" => {
            self.run_command(&context.session_id, "back", vec![], 30).await?;
            Ok(json!({ "success": true }).to_string())
        }
        "browser_press" => {
            let params: PressParams = serde_json::from_value(args)
                .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
            self.run_command(&context.session_id, "press", vec![&params.key], 30).await?;
            Ok(json!({ "success": true, "pressed": params.key }).to_string())
        }
        _ => Err(ToolError::InvalidArgs("Unknown browser command".into())),
    }
}
```

**注意：** 上述代码有问题——`self.name()` 在 async fn 中不能区分不同工具。需要拆分成独立的 impl Tool 块或使用枚举。

### Step 5: 修正架构——使用独立 Tool struct

每个 browser 命令应该是独立的 Tool：

```rust
pub struct BrowserNavigateTool { store: Arc<RwLock<BrowserSessionStore>>, config_dir: PathBuf }
pub struct BrowserSnapshotTool { store: Arc<RwLock<BrowserSessionStore>> }
pub struct BrowserClickTool { store: Arc<RwLock<BrowserSessionStore>> }
// ... 以此类推
```

在 `BrowserToolCore` 中共享会话逻辑：

```rust
/// 共享浏览器核心逻辑（供所有 browser 工具使用）
pub struct BrowserToolCore {
    pub store: Arc<RwLock<BrowserSessionStore>>,
}

impl BrowserToolCore {
    pub fn run_command(&self, task_id: &str, subcmd: &str, args: Vec<&str>, timeout_secs: u64) -> impl Future<Output = Result<serde_json::Value, ToolError>> + '_ {
        async move {
            let (session_name, socket_dir) = {
                let mut store = self.store.write();
                let session = store.get_session(task_id)
                    .ok_or_else(|| ToolError::InvalidArgs("No active session. Call browser_navigate first.".into()))?;
                store.touch(task_id);
                (session.session_name.clone(), session.socket_dir.clone())
            };

            let mut cmd = tokio::process::Command::new("agent-browser");
            cmd.arg("--session").arg(&session_name);
            cmd.arg("--json").arg(subcmd);
            cmd.args(&args);
            cmd.env("AGENT_BROWSER_SOCKET_DIR", &socket_dir);

            let output = tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs),
                cmd.output(),
            )
            .await
            .map_err(|_| ToolError::Execution("Command timed out".into()))?
            .map_err(|e| ToolError::Execution(e.to_string()))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(ToolError::Execution(format!("agent-browser failed: {}", stderr)));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let resp: BrowserResponse = serde_json::from_str(&stdout)
                .map_err(|e| ToolError::Execution(format!("Invalid JSON: {}", e)))?;

            if !resp.success {
                return Err(ToolError::Execution(resp.error.unwrap_or_else(|| "Unknown error".into())));
            }

            Ok(resp.data)
        }
    }
}
```

### Step 6: 注册所有 browser 工具

在 `lib.rs` 中：

```rust
pub mod browser_tools;
pub use browser_tools::{
    BrowserNavigateTool, BrowserSnapshotTool, BrowserClickTool,
    BrowserTypeTool, BrowserScrollTool, BrowserBackTool,
    BrowserPressTool, BrowserVisionTool,
};

pub fn register_builtin_tools(registry: &ToolRegistry) {
    // ... existing tools ...
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config/hermes-agent"));
    let browser_core = browser_tools::BrowserToolCore::new(config_dir);
    registry.register(BrowserNavigateTool::new(browser_core.clone()));
    registry.register(BrowserSnapshotTool::new(browser_core.clone()));
    registry.register(BrowserClickTool::new(browser_core.clone()));
    registry.register(BrowserTypeTool::new(browser_core.clone()));
    registry.register(BrowserScrollTool::new(browser_core.clone()));
    registry.register(BrowserBackTool::new(browser_core.clone()));
    registry.register(BrowserPressTool::new(browser_core.clone()));
    registry.register(BrowserVisionTool::new(browser_core));
}
```

---

## Task 3: browser_vision + 超时清理

**Files:**
- Modify: `crates/hermes-tools-builtin/src/browser_tools.rs`

### Step 1: 实现 browser_vision

```rust
pub struct BrowserVisionTool {
    core: BrowserToolCore,
}

impl BrowserVisionTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: VisionParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // 创建截图路径
        let screenshot_dir = std::env::temp_dir().join("hermes-screenshots");
        std::fs::create_dir_all(&screenshot_dir).ok();
        let screenshot_path = screenshot_dir.join(format!("browser_{}.png", uuid::Uuid::new_v4()));

        // 执行截图
        let mut screenshot_args = vec![];
        if params.annotate {
            screenshot_args.push("--annotate");
        }
        screenshot_args.push("--full");
        screenshot_args.push(screenshot_path.to_str().unwrap());

        self.core.run_command(&context.session_id, "screenshot", screenshot_args, 60).await?;

        // 检查截图是否存在
        if !screenshot_path.exists() {
            return Err(ToolError::Execution("Screenshot file not created".into()));
        }

        // TODO: 调用 VisionTool 分析截图
        // 目前返回截图路径，由调用方决定如何处理
        Ok(json!({
            "success": true,
            "screenshot_path": screenshot_path.to_str(),
            "analysis": "Screenshot captured. Vision analysis not yet integrated."
        }).to_string())
    }
}
```

### Step 2: 实现会话超时清理

在 `BrowserToolCore` 中添加后台清理任务：

```rust
impl BrowserToolCore {
    pub fn new(config_dir: PathBuf) -> Self {
        let core = Self {
            store: Arc::new(RwLock::new(BrowserSessionStore::new())),
        };
        core.start_cleanup_task();
        core
    }

    fn start_cleanup_task(&self) {
        let store = Arc::clone(&self.store);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let stale = {
                    let store = store.read();
                    store.get_stale_sessions()
                };
                if !stale.is_empty() {
                    let mut store = store.write();
                    for task_id in stale {
                        // 关闭会话
                        let session_name = store.task_session_map.get(&task_id).cloned();
                        if let Some(name) = session_name {
                            let _ = tokio::process::Command::new("agent-browser")
                                .arg("--session").arg(&name)
                                .arg("--json").arg("close")
                                .output()
                                .await;
                        }
                        store.remove_session(&task_id);
                    }
                }
            }
        });
    }
}
```

---

## Task 4: 集成验证

**Files:**
- Modify: `crates/hermes-tools-builtin/src/lib.rs`

### Step 1: 编译检查

Run: `cargo check -p hermes-tools-builtin 2>&1 | tail -30`

### Step 2: 测试运行

Run: `cargo test -p hermes-tools-builtin test_browser -- --nocapture 2>&1 | tail -30`

### Step 3: 提交

```bash
git add crates/hermes-tools-builtin/src/browser_tools.rs crates/hermes-tools-builtin/src/lib.rs crates/hermes-tools-builtin/tests/test_browser.rs
git commit -m "feat(tools-builtin): add BrowserTool for browser automation

- Session management via agent-browser CLI
- Basic navigation, snapshot, click, type, scroll, back, press
- Session timeout cleanup (5 min inactivity)
- browser_vision (screenshot capture)

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## 验收清单

### 基础功能
- [ ] `BrowserSessionStore` create/get/remove/touch 工作正常
- [ ] `agent-browser --version` 成功（在有安装的环境中）
- [ ] `browser_navigate` 创建会话并返回 snapshot
- [ ] `browser_snapshot` 返回可点击元素的 ref IDs
- [ ] `browser_click` 正常工作
- [ ] `browser_type` 正常工作
- [ ] `browser_scroll` 正常工作

### Session 管理
- [ ] Session 5 分钟超时自动清理
- [ ] 并发安全（RwLock）

### 高级功能
- [ ] `browser_vision` 返回截图路径
- [ ] VisionTool 集成（如 hermes-tools-extended 可用）

### 集成
- [ ] 所有 browser 工具在 `register_builtin_tools` 中注册
- [ ] `cargo check --all` 通过
- [ ] `cargo test -p hermes-tools-builtin` 通过

---

## 关键类型对照

| 类型/方法 | 定义位置 |
|-----------|----------|
| `Tool` trait | `hermes-tool-registry/src/lib.rs` |
| `ToolContext`, `ToolError` | `hermes-core/src/lib.rs` |
| `BrowserSessionStore` | `crates/hermes-tools-builtin/src/browser_tools.rs` |
| `BrowserToolCore` | `crates/hermes-tools-builtin/src/browser_tools.rs` |
| `VisionTool` | `hermes-tools-extended/src/vision.rs`（非 builtin） |
