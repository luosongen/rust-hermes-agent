# Rust-Python Parity: Missing Tools Implementation Plan

> **Status:** Draft
> **Date:** 2026-04-16
> **Scope:** 5 new tools to close the gap with Python hermes-agent
> **Local-first:** All tools work without cloud dependencies; cloud features are additive

---

## 概述

本文档定义 5 个缺失工具的设计，实现 Rust 版 hermes-agent 与 Python 版的功能对齐。

**设计原则：**
- 本地优先：核心功能不依赖外部云服务
- 云 API 可选：搜索、视觉分析等支持可配置的云后端
- 增量实现：每个工具独立开发和测试
- 架构一致性：遵循 Rust 版的现有模式（Tool trait、BrowserToolCore 共享模式等）

---

## 实现顺序

1. **VisionTool** — 独立图像分析（补全 browser_vision 的分析能力）
2. **MemoryTool** — 持久化记忆（文件快照 + SQLite FTS 检索）
3. **DelegateTool** — 子 Agent 并发执行（独立进程 CLI）
4. **WebSearchTool** — 网页搜索（Exa + Tavily + Firecrawl，云端搜索 + 本地 LLM summarization）
5. **CodeExecutionTool** — 代码执行（PTC：UDS 本地 + 文件 RPC 远程）

---

## 1. VisionTool

### 目标

提供独立图像分析工具，接受 URL 或 base64 编码图像，返回 LLM 视觉分析结果。可被 `browser_vision` 集成使用。

### 核心类型

```rust
// VisionTool — 单例工具
pub struct VisionTool {
    http_client: reqwest::Client,
}

// VisionProvider — 可插拔的视觉分析后端
pub trait VisionProvider: Send + Sync {
    async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError>;
}

pub enum VisionImage {
    Url(String),       // 自动下载
    Base64(String),   // 直接 base64
}

pub struct VisionResult {
    pub analysis: String,
    pub model: String,
}
```

### 内置 Provider

```rust
// OpenAIVisionProvider — GPT-4V
pub struct OpenAIVisionProvider {
    api_key: String,
    model: String, // "gpt-4o", "gpt-4o-mini"
    http_client: reqwest::Client,
}

// AnthropicVisionProvider — Claude Vision
pub struct AnthropicVisionProvider {
    api_key: String,
    model: String, // "claude-3-5-sonnet-20241022"
    http_client: reqwest::Client,
}
```

### 工具接口

```json
{
  "name": "vision_analyze",
  "description": "Analyze image with vision AI. Accepts URL (auto-download) or base64-encoded image.",
  "parameters": {
    "type": "object",
    "properties": {
      "image": { "type": "string", "description": "Image URL or base64" },
      "question": { "type": "string", "description": "Question about the image" },
      "provider": { "type": "string", "enum": ["openai", "anthropic"], "default": "openai" }
    },
    "required": ["image", "question"]
  }
}
```

### 响应格式

```json
{
  "success": true,
  "analysis": "The image shows a person standing...",
  "model": "gpt-4o"
}
```

### 文件结构

```
crates/hermes-tools-extended/src/
├── vision.rs              # VisionTool + VisionProvider trait
└── vision_providers.rs   # OpenAI, Anthropic providers
```

### 实现步骤

1. `VisionProvider` trait 定义
2. `OpenAIVisionProvider` 实现（调用 OpenAI vision API）
3. `AnthropicVisionProvider` 实现（调用 Anthropic vision API）
4. URL 下载 + base64 解码
5. `VisionTool` 集成到 `register_extended_tools`
6. 补全 `browser_vision` 调用 `VisionTool` 做分析

---

## 2. MemoryTool

### 目标

提供持久化记忆能力，支持文件快照和语义检索。记忆注入到对话 context 前由 LLM 压缩。

### 核心类型

```rust
// MemoryTool — 单例
pub struct MemoryTool {
    store: Arc<RwLock<MemoryStore>>,
    config_dir: PathBuf,
}

pub struct MemoryStore {
    // 文件快照（TEXT 模式）
    snapshot_path: PathBuf,
    // SQLite FTS 检索
    conn: rusqlite::Connection,
}
```

### 工具接口

```json
{
  "name": "memory_add",
  "description": "Add observation to persistent memory.",
  "parameters": {
    "type": "object",
    "properties": {
      "content": { "type": "string" },
      "category": { "type": "string", "default": "note" },
      "tags": { "type": "array", "items": { "type": "string" }, "default": [] }
    },
    "required": ["content"]
  }
}
```

```json
{
  "name": "memory_search",
  "description": "Search persistent memory by keywords.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": { "type": "string" },
      "limit": { "type": "integer", "default": 10 }
    },
    "required": ["query"]
  }
}
```

```json
{
  "name": "memory_read",
  "description": "Read all memories as a formatted string for context injection.",
  "parameters": {
    "type": "object",
    "properties": {
      "category": { "type": "string" }
    }
  }
}
```

### SQLite FTS 模式

```sql
CREATE VIRTUAL TABLE memory_fts USING fts5(
    content,
    category,
    tags,
    content_rowid='rowid'
);
```

### 文件结构

```
crates/hermes-tools-extended/src/
└── memory_tool.rs   # MemoryTool + MemoryStore
```

---

## 3. DelegateTool

### 目标

启动独立 hermes-cli 子进程执行子任务，支持并发和受限工具集。Python 版的 `delegate_tool.py` 在 Rust 中的对等实现。

### 核心类型

```rust
// DelegateTool — 单例
pub struct DelegateTool {
    cli_path: PathBuf,
    config_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateParams {
    pub goal: String,                    // 子任务描述
    pub toolsets: Vec<String>,          // 允许的工具集
    #[serde(default)]
    pub max_iterations: Option<u32>,
    #[serde(default)]
    pub context: Option<String>,        // 额外上下文
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateResult {
    pub status: String,    // "success", "failed", "cancelled"
    pub summary: String,    // LLM 生成的结果摘要
    pub duration_secs: f64,
}
```

### CLI 通信协议

```
父进程 → 子进程（stdin）:
{"jsonrpc": "2.0", "method": "start", "params": {...}}
{"jsonrpc": "2.0", "method": "stop"}

子进程 → 父进程（stdout）:
{"jsonrpc": "2.0", "result": {"status": "success", "summary": "..."}}
```

### 工具接口

```json
{
  "name": "delegate_task",
  "description": "Delegate a task to a subagent with isolated context.",
  "parameters": {
    "type": "object",
    "properties": {
      "goal": { "type": "string" },
      "toolsets": { "type": "array", "items": { "type": "string" } },
      "max_iterations": { "type": "integer" },
      "context": { "type": "string" }
    },
    "required": ["goal", "toolsets"]
  }
}
```

### 工具集过滤

子进程只能访问 `toolsets` 中列出的工具集，并移除 `DELEGATE_BLOCKED_TOOLS`（`delegate_task`, `clarify`, `memory`, `send_message`, `execute_code`）。

### 并发控制

通过 `tokio::sync::Semaphore` 限制并发子进程数（默认 3，可配置）。

### 文件结构

```
crates/hermes-tools-extended/src/
└── delegate_tool.rs   # DelegateTool + CLI 通信
```

---

## 4. WebSearchTool

### 目标

提供网页搜索和内容提取能力，调用云搜索 API + 本地 LLM summarization。

### 核心类型

```rust
pub struct WebSearchTool {
    providers: Vec<Box<dyn SearchProvider>>,
    llm_summarizer: LlmSummarizer,
}

pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError>;
    fn name(&self) -> &str;
}

pub struct SearchResult {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub content: Option<String>,  // extracted full content
}
```

### 内置 Provider

```rust
// ExaSearchProvider — Exa AI (exa.ai)
pub struct ExaSearchProvider { api_key: String }

// TavilySearchProvider — Tavily AI (tavily.ai)
pub struct TavilySearchProvider { api_key: String }

// FirecrawlSearchProvider — Firecrawl (firecrawl.dev)
pub struct FirecrawlSearchProvider { api_key: String, engine: String }
```

### LLM Summarizer

搜索结果通过 `hermes-provider` 的 LLM 做 summarization，避免将大量原始内容填满 context。

```rust
pub struct LlmSummarizer {
    provider: Arc<dyn LlmProvider>,
}
```

### 工具接口

```json
{
  "name": "web_search",
  "description": "Search the web and optionally extract content from results.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": { "type": "string" },
      "provider": { "type": "string", "enum": ["exa", "tavily", "firecrawl"], "default": "exa" },
      "extract_content": { "type": "boolean", "default": false }
    },
    "required": ["query"]
  }
}
```

### 响应格式

```json
{
  "success": true,
  "query": "rust async runtime comparison",
  "results": [
    {
      "url": "https://example.com/article",
      "title": "Rust Async Runtime Deep Dive",
      "snippet": "...",
      "content": "...",
      "summary": "This article compares tokio, async-std, and smol..."
    }
  ],
  "provider": "exa"
}
```

### 文件结构

```
crates/hermes-tools-extended/src/
├── web_search.rs        # WebSearchTool + SearchProvider trait
└── search_providers.rs  # Exa, Tavily, Firecrawl providers
```

---

## 5. CodeExecutionTool

### 目标

PTC（Programmatic Tool Calling）：让 LLM 写 Python 脚本，通过 RPC 调用 hermes 工具，单次推理内完成多步工具调用。

### 架构

**本地 UDS 模式：**
```
LLM 脚本 → Unix Domain Socket → 父进程 → Hermes 工具 → 结果
```

**远程文件 RPC 模式：**
```
LLM 脚本 → 写请求文件 → 父进程 polling → Hermes 工具 → 写响应文件
```

### 核心类型

```rust
pub struct CodeExecutionTool {
    store: Arc<RwLock<ExecutionStore>>,
    config: ExecutionConfig,
}

#[derive(Debug, Deserialize)]
pub struct ExecutionConfig {
    pub allowed_tools: Vec<String>,    // 默认 ["web_search", "web_extract", "read_file", "write_file", "search_files", "patch", "terminal"]
    pub timeout_secs: u64,            // 默认 300
    pub max_tool_calls: u32,          // 默认 50
    pub max_stdout_bytes: usize,      // 默认 50_000
    pub max_stderr_bytes: usize,      // 默认 10_000
}

pub struct ExecutionStore {
    pending: HashMap<String, ExecutionHandle>,
}

pub struct ExecutionHandle {
    pub task_id: String,
    pub status: ExecutionStatus,
    pub start_time: f64,
}
```

### hermes_tools.py 生成

父进程为子脚本生成 `hermes_tools.py` stub，包含工具 RPC 函数：

```python
# UDS 模式
def read_file(path: str) -> dict:
    # 调用父进程 UDS RPC
    pass

# 文件 RPC 模式
def read_file(path: str) -> dict:
    # 写请求文件，poll 响应文件
    pass
```

### 工具接口

```json
{
  "name": "execute_code",
  "description": "Execute Python code that calls Hermes tools via RPC. Returns stdout.",
  "parameters": {
    "type": "object",
    "properties": {
      "code": { "type": "string" },
      "language": { "type": "string", "enum": ["python"], "default": "python" },
      "timeout_secs": { "type": "integer" }
    },
    "required": ["code"]
  }
}
```

### 响应格式

```json
{
  "success": true,
  "stdout": "Analysis complete. Found 42 files...",
  "stderr": "",
  "tool_calls": 12,
  "duration_secs": 8.3
}
```

### 文件结构

```
crates/hermes-tools-extended/src/
└── code_execution.rs   # CodeExecutionTool + RPC 生成
```

---

## 依赖

```toml
# hermes-tools-extended/Cargo.toml
reqwest = { workspace = true, features = ["json"] }
rusqlite = { workspace = true, features = ["bundled"] }
tokio = { workspace = true, features = ["process", "fs", "io-util"] }
```

---

## 实现顺序与文件位置

| 工具 | Crate | 优先 |
|------|-------|------|
| VisionTool | hermes-tools-extended | 1 |
| MemoryTool | hermes-tools-extended | 2 |
| DelegateTool | hermes-tools-extended | 3 |
| WebSearchTool | hermes-tools-extended | 4 |
| CodeExecutionTool | hermes-tools-extended | 5 |

---

## 与 Python 版的主要差异

| 方面 | Python 版 | Rust 版（本文） |
|------|----------|----------------|
| Vision | OpenAI + Anthropic + Google | OpenAI + Anthropic（相同） |
| Memory | 文件注入 + LLM 压缩 | 文件快照 + SQLite FTS |
| Delegate | ThreadPoolExecutor + ACP | 独立进程 CLI + JSON-RPC |
| WebSearch | Exa + Tavily + Firecrawl + Parallel | Exa + Tavily + Firecrawl（相同） |
| CodeExecution | UDS + 文件 RPC | UDS + 文件 RPC（相同） |
| 并发 | ThreadPoolExecutor | tokio::sync::Semaphore + tokio::process |
