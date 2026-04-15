# Phase 3: Tools + MCP Design Specification

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现扩展工具集（Web Search、Web Fetch、Cron Scheduler）和 MCP Server Bridge

**Architecture:**
- 新建 `hermes-tools-extended` crate 存放扩展工具
- 遵循现有 `hermes-tools-builtin` 的 Tool trait 模式
- MCP Server Bridge 实现 JSON-RPC 2.0 over stdio

**Tech Stack:** Rust, reqwest, tokio, serde_json, cron

---

## 模块结构

```
crates/
├── hermes-tools-extended/          # 新建
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                 # 导出所有工具
│       ├── web_search.rs           # 网页搜索
│       ├── web_fetch.rs            # 网页抓取
│       ├── cron_scheduler.rs       # 定时任务
│       └── mcp_server.rs           # MCP Server Bridge
│
├── hermes-tools-builtin/          # 已有
│   └── src/
│       ├── file_tools.rs
│       ├── terminal_tools.rs
│       └── skills.rs
│
└── hermes-tool-registry/          # 已有
    └── src/
        └── registry.rs            # Tool trait 定义
```

---

## Tool trait 接口（来自 hermes-tool-registry）

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError>;
}
```

---

## Task 1: WebSearchTool

**Files:**
- Create: `crates/hermes-tools-extended/src/web_search.rs`

### WebSearchTool 实现

```rust
pub struct WebSearchTool {
    // HTTP client
}

impl WebSearchTool {
    pub fn new() -> Self;
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str { "Search the web for information" }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "num_results": { "type": "integer", "default": 10 }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError>;
}
```

### API 选择
- 主要：使用 DuckDuckGo HTML API（免费，无需 API Key）
- 备选：Google SerpAPI（需要 Key）

---

## Task 2: WebFetchTool

**Files:**
- Create: `crates/hermes-tools-extended/src/web_fetch.rs`

### WebFetchTool 实现

```rust
pub struct WebFetchTool {
    // HTTP client
}

impl WebFetchTool {
    pub fn new() -> Self;
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }
    fn description(&self) -> &str { "Fetch and extract content from a URL" }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "URL to fetch" },
                "extract_pattern": { "type": "string", "description": "Optional regex to extract specific content" }
            },
            "required": ["url"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError>;
}
```

---

## Task 3: CronScheduler

**Files:**
- Create: `crates/hermes-tools-extended/src/cron_scheduler.rs`

### CronScheduler 实现

```rust
pub struct CronScheduler {
    // tokio runtime handle
    // SQLite store for persisted jobs
}

impl CronScheduler {
    pub fn new() -> Self;
    pub fn schedule(&self, cron_expr: &str, tool_name: &str, args: serde_json::Value) -> Result<String, String>;
    pub fn cancel(&self, job_id: &str) -> Result<(), String>;
    pub fn list(&self) -> Vec<ScheduledJob>;
}

#[async_trait]
impl Tool for CronScheduler {
    fn name(&self) -> &str { "cron_schedule" }
    fn description(&self) -> &str { "Schedule a tool to run at a later time" }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "cron_expression": { "type": "string" },
                "tool_name": { "type": "string" },
                "tool_args": { "type": "object" }
            },
            "required": ["cron_expression", "tool_name"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError>;
}
```

### Cron 表达式格式
支持标准 5 字段格式: `分 时 日 月 周`
例: `0 9 * * *` = 每天 9:00

---

## Task 4: McpServerBridge

**Files:**
- Create: `crates/hermes-tools-extended/src/mcp_server.rs`

### McpServerBridge 实现

MCP 协议使用 JSON-RPC 2.0 over stdio。

```rust
pub struct McpServerBridge {
    tool_registry: Arc<ToolRegistry>,
    input: Stdin,
    output: Stdout,
}

impl McpServerBridge {
    pub fn new(registry: Arc<ToolRegistry>) -> Self;
    
    /// 启动 MCP 服务器主循环
    pub async fn run(&self) -> Result<(), String>;
    
    /// 处理单个 JSON-RPC 请求
    async fn handle_request(&self, request: McpRequest) -> Result<McpResponse, String>;
}
```

### MCP 协议方法

| 方法 | 方向 | 描述 |
|------|------|------|
| `initialize` | Client→Server | 初始化连接 |
| `tools/list` | Client→Server | 列出可用工具 |
| `tools/call` | Client→Server | 调用工具 |
| `notifications/*` | 双向 | 事件通知 |

### 响应格式

```json
// tools/list 响应
{
  "jsonrpc": "2.0",
  "result": {
    "tools": [
      { "name": "read_file", "description": "...", "inputSchema": {...} }
    ]
  }
}
```

---

## Task 5: hermes-tools-extended crate 初始化

**Files:**
- Create: `crates/hermes-tools-extended/Cargo.toml`
- Create: `crates/hermes-tools-extended/src/lib.rs`
- Modify: `Cargo.toml` (workspace)

### Cargo.toml 依赖

```toml
[package]
name = "hermes-tools-extended"
version.workspace = true
edition.workspace = true

[dependencies]
tokio.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
async-trait.workspace = true
parking_lot.workspace = true
hermes-core.workspace = true
hermes-tool-registry.workspace = true
scraper = "0.21"  # HTML parsing
regex = "1"        # Pattern extraction
cron = "0.15"     # Cron parsing
```

---

## 实现顺序

1. **Task 5**: 创建 `hermes-tools-extended` crate 结构
2. **Task 1**: WebSearchTool + 单元测试
3. **Task 2**: WebFetchTool + 单元测试
4. **Task 3**: CronScheduler + 单元测试
5. **Task 4**: McpServerBridge + 集成测试

---

## 验收清单

- [ ] `hermes-tools-extended` crate 编译通过
- [ ] WebSearchTool 单元测试通过
- [ ] WebFetchTool 单元测试通过
- [ ] CronScheduler 单元测试通过
- [ ] McpServerBridge JSON-RPC 协议测试通过
- [ ] `cargo check --all` 通过

---

## 关键文件

| 文件 | 职责 |
|------|------|
| `crates/hermes-tools-extended/Cargo.toml` | 新 crate 配置 |
| `crates/hermes-tools-extended/src/lib.rs` | 模块导出 |
| `crates/hermes-tools-extended/src/web_search.rs` | 网页搜索 |
| `crates/hermes-tools-extended/src/web_fetch.rs` | 网页抓取 |
| `crates/hermes-tools-extended/src/cron_scheduler.rs` | 定时任务 |
| `crates/hermes-tools-extended/src/mcp_server.rs` | MCP 协议服务 |

---

## 下一步 (Phase 3.5)

- MCP Client 实现 — 连接外部 MCP 服务器
- Code Execution — 沙箱代码执行（安全评估后决定）
