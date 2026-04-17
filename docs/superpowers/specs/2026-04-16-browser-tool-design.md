# BrowserTool Design Spec

> **Status:** Approved
> **Date:** 2026-04-16
> **Scope:** Local-only (agent-browser CLI + headless Chromium)

---

## 概述

BrowserTool 是 hermes-agent 的浏览器自动化工具，通过 `agent-browser` CLI 调用本地 headless Chromium，支持页面导航、交互和内容提取。

**限制：** 仅本地模式。不支持云端后端（Browserbase、Browser Use）。

---

## 核心类型

```rust
// BrowserTool — 单例，管理浏览器会话生命周期
pub struct BrowserTool {
    store: Arc<RwLock<BrowserSessionStore>>,
    config_dir: PathBuf,
}

// 会话存储
#[derive(Default)]
pub struct BrowserSessionStore {
    sessions: HashMap<String, BrowserSession>,
    task_session_map: HashMap<String, String>, // task_id -> session_name
}

pub struct BrowserSession {
    session_name: String,
    task_id: String,
    socket_dir: PathBuf,
    created_at: f64,
    last_activity: f64,
}
```

---

## 工具接口

**工具集前缀:** `browser_`

### browser_navigate

```json
{
  "name": "browser_navigate",
  "description": "Navigate to a URL. Initializes session and loads page. Returns compact snapshot automatically.",
  "parameters": {
    "type": "object",
    "properties": {
      "url": { "type": "string" }
    },
    "required": ["url"]
  }
}
```

### browser_snapshot

```json
{
  "name": "browser_snapshot",
  "description": "Get text-based accessibility tree snapshot. compact (default): interactive elements only. full: complete page.",
  "parameters": {
    "type": "object",
    "properties": {
      "full": { "type": "boolean", "default": false }
    }
  }
}
```

### browser_click

```json
{
  "name": "browser_click",
  "description": "Click element by ref ID (e.g., '@e5').",
  "parameters": {
    "type": "object",
    "properties": {
      "ref": { "type": "string" }
    },
    "required": ["ref"]
  }
}
```

### browser_type

```json
{
  "name": "browser_type",
  "description": "Type text into input field by ref ID. Clears field first.",
  "parameters": {
    "type": "object",
    "properties": {
      "ref": { "type": "string" },
      "text": { "type": "string" }
    },
    "required": ["ref", "text"]
  }
}
```

### browser_scroll

```json
{
  "name": "browser_scroll",
  "description": "Scroll page up or down.",
  "parameters": {
    "type": "object",
    "properties": {
      "direction": { "type": "string", "enum": ["up", "down"] }
    },
    "required": ["direction"]
  }
}
```

### browser_back

```json
{
  "name": "browser_back",
  "description": "Navigate back in browser history.",
  "parameters": {
    "type": "object",
    "properties": {}
  }
}
```

### browser_press

```json
{
  "name": "browser_press",
  "description": "Press keyboard key (Enter, Tab, Escape, ArrowDown, etc.).",
  "parameters": {
    "type": "object",
    "properties": {
      "key": { "type": "string" }
    },
    "required": ["key"]
  }
}
```

### browser_vision

```json
{
  "name": "browser_vision",
  "description": "Take screenshot and analyze with vision AI.",
  "parameters": {
    "type": "object",
    "properties": {
      "question": { "type": "string" },
      "annotate": { "type": "boolean", "default": false }
    },
    "required": ["question"]
  }
}
```

---

## 架构

```
BrowserTool (单例)
  └── BrowserSessionStore (Arc<RwLock>)
        ├── sessions: HashMap<session_name, BrowserSession>
        └── task_session_map: HashMap<task_id, session_name>

会话生命周期:
  browser_navigate(url) → 创建会话 → agent-browser open <url>
  其他操作 → agent-browser <command> --session <session_name>
  cleanup / 超时 → agent-browser close --session <session_name>
```

---

## agent-browser CLI 接口

**安装:** `npm install -g agent-browser && agent-browser install`

**会话模式:**
```bash
agent-browser --session <name> --json open <url>
agent-browser --session <name> --json snapshot [-c]
agent-browser --session <name> --json click <ref>
agent-browser --session <name> --json fill <ref> <text>
agent-browser --session <name> --json scroll <up|down> <pixels>
agent-browser --session <name> --json back
agent-browser --session <name> --json press <key>
agent-browser --session <name> --json screenshot [--full] <path>
agent-browser --session <name> --json close
```

**输出格式:** JSON `{ "success": bool, "data": {...}, "error": string? }`

---

## 会话管理

### 创建会话

```rust
fn create_session(task_id: &str) -> BrowserSession {
    let session_name = format!("h_{}", uuid_short());
    let socket_dir = tmpdir / format!("agent-browser-{}", session_name);
    // agent-browser 会在此目录创建 Unix socket 和 PID file
    BrowserSession { session_name, task_id, socket_dir, created_at: now(), last_activity: now() }
}
```

### tokio::process 管理

```rust
async fn run_command(&self, task_id: &str, cmd: &str, args: &[&str]) -> Result<Value> {
    let session = self.store.read().get_session(task_id)?;
    let mut cmd = tokio::process::Command::new("agent-browser");
    cmd.arg("--session").arg(&session.session_name);
    cmd.arg("--json").arg(cmd);
    cmd.args(args);
    cmd.env("AGENT_BROWSER_SOCKET_DIR", &session.socket_dir);
    // stdout = file, stderr = file (避免 PIPE 阻塞)
    // timeout via tokio::time::timeout
}
```

### 超时清理

```rust
// 5 分钟无活动视为超时
const INACTIVITY_TIMEOUT_SECS: u64 = 300;

// 定期检查: tokio::spawn(async move { loop { sleep(30s).await; cleanup_stale(); } })
```

---

## 响应格式

### navigate 响应

```json
{
  "success": true,
  "url": "https://example.com",
  "title": "Example Domain",
  "snapshot": "[ref=e1] Sign in button\n[ref=e2] Search input",
  "element_count": 2
}
```

### snapshot 响应

```json
{
  "success": true,
  "snapshot": "[ref=e1] Button: Submit\n[ref=e2] Input: Username",
  "element_count": 2
}
```

### vision 响应

```json
{
  "success": true,
  "analysis": "The page shows a login form...",
  "screenshot_path": "/tmp/hermes-screenshots/browser_xxx.png"
}
```

---

## 错误处理

| 错误 | 处理 |
|------|------|
| agent-browser not found | 返回错误 "agent-browser CLI not installed" |
| Session not found | 自动创建新会话 |
| Command timeout | 杀掉进程，返回超时错误 |
| Navigation failed | 返回 `success: false, error: "..."` |

---

## 文件结构

```
crates/hermes-tools-builtin/src/
├── browser_tools.rs      # BrowserTool + BrowserSessionStore

crates/hermes-tools-builtin/tests/
└── test_browser.rs       # 单元测试
```

---

## 实现计划（Task）

### Task 1: 核心类型 + session 管理

- BrowserSessionStore (HashMap 管理)
- create_session / get_session / cleanup_session
- tokio::process::Command 包装
- `agent-browser --session <name> --json` 调用封装
- 响应 JSON 解析

### Task 2: 基础 browser 工具

- browser_navigate
- browser_snapshot
- browser_click
- browser_type
- browser_scroll
- browser_back
- browser_press

### Task 3: 高级工具

- browser_vision (截图 + VisionTool 分析)
- Session 超时清理 (后台任务)

### Task 4: 集成

- 注册到 `register_builtin_tools`
- 错误处理完善
- 测试通过

---

## 依赖

```toml
# Cargo.toml (hermes-tools-builtin)
uuid.workspace = true
tokio = { workspace = true, features = ["process", "time", "sync", "rt"] }
```

---

## 验收清单

- [ ] `agent-browser --version` 成功
- [ ] navigate 创建会话并返回 snapshot
- [ ] snapshot 返回可点击元素的 ref IDs
- [ ] click/type/scroll 正常工作
- [ ] vision 返回截图路径和 AI 分析
- [ ] Session 5 分钟超时自动清理
- [ ] 并发安全（ RwLock）
