# VisionTool + MemoryTool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现两个新工具 — VisionTool（图像分析）和 MemoryTool（跨会话 K-V 记忆）

**Architecture:**
- VisionTool 放在 `hermes-tools-extended/src/vision.rs`，复用 `hermes-core` 已有的 `Content::Image` 类型，通过 `LlmProvider::chat()` 发送多模态请求
- MemoryTool 放在 `hermes-tools-extended/src/memory.rs`，持有 `Arc<SqliteSessionStore>` 引用，在同一 SQLite 数据库上创建 `memory` 表

**Tech Stack:** Rust async/await, async_trait, reqwest, hermes-core, hermes-memory

---

## 文件结构

```
crates/hermes-tools-extended/src/
├── vision.rs          # VisionTool 实现（新建）
├── memory.rs          # MemoryTool 实现（新建）
└── lib.rs             # 模块导出 + register_extended_tools 更新

crates/hermes-memory/src/
├── sqlite_store.rs     # 新增 memory 表创建逻辑
└── memory_manager.rs  # MemoryManager（新建）
```

---

## Task 1: VisionTool 核心实现

**Files:**
- Create: `crates/hermes-tools-extended/src/vision.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`
- Test: `crates/hermes-tools-extended/tests/test_vision.rs`

### 步骤 1: 写测试

```rust
// crates/hermes-tools-extended/tests/test_vision.rs
use hermes_tools_extended::VisionTool;

#[tokio::test]
async fn test_vision_tool_name() {
    let tool = VisionTool::new();
    assert_eq!(tool.name(), "vision");
}

#[tokio::test]
async fn test_vision_parameters_schema() {
    let tool = VisionTool::new();
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    let props = params.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("image"));
    assert!(props.contains_key("prompt"));
}
```

### 步骤 2: 运行测试确认失败

Run: `cargo test -p hermes-tools-extended test_vision_tool_name`
Expected: FAIL — module not found

### 步骤 3: 创建 vision.rs 骨架

```rust
//! VisionTool — 图像分析工具
//!
//! 调用云服务视觉模型（GPT-4V / Claude Vision / Gemini）分析图像内容。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError, Content};
use hermes_tool_registry::Tool;
use serde_json::json;
use std::sync::Arc;

/// VisionTool — 图像分析工具
#[derive(Debug, Clone)]
pub struct VisionTool {
    provider: Arc<dyn LlmProvider>,
}

impl VisionTool {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str { "vision" }

    fn description(&self) -> &str {
        "Analyze images using vision-capable LLM models. Supports image URLs and local file paths."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Image URL or local file path"
                },
                "prompt": {
                    "type": "string",
                    "description": "Analysis instruction",
                    "default": "Describe this image in detail"
                },
                "model": {
                    "type": "string",
                    "description": "Optional: vision model name override"
                }
            },
            "required": ["image"]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        // 实现见步骤 4
    }
}
```

### 步骤 4: 实现 execute 方法

```rust
async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
    let image = args["image"]
        .as_str()
        .ok_or_else(|| ToolError::InvalidArgs("image is required".to_string()))?;
    let prompt = args["prompt"]
        .as_str()
        .unwrap_or("Describe this image in detail");

    // 解析 image：如果是本地路径则读取并转为 base64；如果是 URL 则直接使用
    let content = if image.starts_with("http://") || image.starts_with("https://") {
        Content::Image { url: image.to_string(), detail: None }
    } else {
        // 本地文件：读取并 base64 编码
        let data = std::fs::read(image)
            .map_err(|e| ToolError::Execution(format!("Failed to read image file: {}", e)))?;
        let base64 = base64_encode(&data);
        let mime = guess_mime(image);
        Content::Image {
            url: format!("data:{};base64,{}", mime, base64),
            detail: None,
        }
    };

    let message = hermes_core::Message::user(content);

    let request = hermes_core::ChatRequest {
        model: hermes_core::ModelId::new("openai", "gpt-4o"),
        messages: vec![message],
        tools: None,
        system_prompt: None,
        temperature: None,
        max_tokens: None,
    };

    let response = self.provider.chat(request).await
        .map_err(|e| ToolError::Execution(e.to_string()))?;

    if response.content.is_empty() {
        return Err(ToolError::Execution("Empty response from vision model".to_string()));
    }

    Ok(response.content)
}
```

### 步骤 5: 更新 lib.rs

在 `crates/hermes-tools-extended/src/lib.rs` 中添加：

```rust
pub mod vision;
pub mod memory;
pub use vision::VisionTool;
pub use memory::MemoryTool;
```

并更新 `register_extended_tools`：

```rust
pub fn register_extended_tools(registry: &ToolRegistry, llm_provider: Arc<dyn LlmProvider>) {
    registry.register(WebSearchTool::new());
    registry.register(WebFetchTool::new());
    registry.register(CronScheduler::new());
    registry.register(CliExecutor::new(ExecutorConfig::default()));
    registry.register(VisionTool::new(llm_provider));
    registry.register(MemoryTool::new(session_store.clone()));
}
```

注意：`MemoryTool` 需要 `Arc<SqliteSessionStore>`，这会在 Task 3 实现。

### 步骤 6: 运行测试确认通过

Run: `cargo test -p hermes-tools-extended test_vision`
Expected: PASS

### 步骤 7: 提交

```bash
git add crates/hermes-tools-extended/src/vision.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(tools-extended): add VisionTool for image analysis"
```

---

## Task 2: LlmProvider 多模态支持验证

**Files:**
- Modify: `crates/hermes-provider/src/openai.rs`
- Check: `crates/hermes-core/src/types.rs`

### 步骤 1: 检查 OpenAiProvider 是否支持 Content::Image

Run: `grep -n "Content::Image\|Image\|image_url\|vision" crates/hermes-provider/src/openai.rs | head -20`
Expected: 确认 OpenAiProvider 的 chat 方法能处理包含 `Content::Image` 的 Message

### 步骤 2: 如果不支持，修改 OpenAiProvider 的 chat 方法

查看当前 `openai.rs` 中 `chat` 方法如何序列化 `Message.content`：

找到将 `Content` 序列化为 OpenAI API 格式的代码位置，确保 `Content::Image { url, detail }` 被正确转换为 OpenAI 的 `image_url` 或 `image_base64` 格式。

### 步骤 3: 提交

```bash
git add crates/hermes-provider/src/openai.rs
git commit -m "feat(provider): ensure OpenAiProvider handles Content::Image for vision requests"
```

---

## Task 3: MemoryTool 核心实现

**Files:**
- Create: `crates/hermes-tools-extended/src/memory.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`
- Modify: `crates/hermes-memory/src/sqlite_store.rs` (添加 memory 表)
- Test: `crates/hermes-tools-extended/tests/test_memory.rs`

### 步骤 1: 写测试

```rust
// crates/hermes-tools-extended/tests/test_memory.rs
use hermes_tools_extended::MemoryTool;

#[tokio::test]
async fn test_memory_tool_name() {
    // 需要 mock SqliteSessionStore
    // 暂时跳过具体实现，验证类型存在即可
}

#[tokio::test]
async fn test_memory_parameters() {
    let tool = MemoryTool::new(/* mock store */);
    let params = tool.parameters();
    assert!(params.get("oneOf").is_some());
}
```

### 步骤 2: 添加 memory 表到 SqliteSessionStore

在 `crates/hermes-memory/src/sqlite_store.rs` 的 `SCHEMA` 常量末尾添加：

```rust
CREATE TABLE IF NOT EXISTS memory (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at REAL NOT NULL,
    updated_at REAL NOT NULL
);
```

### 步骤 3: 创建 memory.rs

```rust
//! MemoryTool — 跨会话持久化记忆工具
//!
//! 提供 K-V 存储，支持 set / get / search 三种操作。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// MemoryTool — 跨会话持久化记忆工具
#[derive(Debug, Clone)]
pub struct MemoryTool {
    store: Arc<SqliteSessionStore>,
}

impl MemoryTool {
    pub fn new(store: Arc<SqliteSessionStore>) -> Self {
        Self { store }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum MemoryParams {
    Set { key: String, value: String },
    Get { key: String },
    Search { query: String, limit: Option<usize> },
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str { "memory" }

    fn description(&self) -> &str {
        "Cross-session persistent memory. Supports set(key, value), get(key), search(query)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "action": { "const": "set" },
                        "key": { "type": "string" },
                        "value": { "type": "string" }
                    },
                    "required": ["action", "key", "value"]
                },
                {
                    "properties": {
                        "action": { "const": "get" },
                        "key": { "type": "string" }
                    },
                    "required": ["action", "key"]
                },
                {
                    "properties": {
                        "action": { "const": "search" },
                        "query": { "type": "string" },
                        "limit": { "type": "integer", "default": 5 }
                    },
                    "required": ["action", "query"]
                }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: MemoryParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params {
            MemoryParams::Set { key, value } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as f64;
                sqlx::query(
                    "INSERT OR REPLACE INTO memory (key, value, created_at, updated_at) VALUES (?, ?, COALESCE((SELECT created_at FROM memory WHERE key = ?), ?), ?)"
                )
                .bind(&key)
                .bind(&value)
                .bind(&key)
                .bind(now)
                .bind(now)
                .execute(self.store.pool())
                .await
                .map_err(|e| ToolError::Execution(format!("Memory set error: {}", e)))?;
                Ok(json!({ "status": "ok", "key": key }).to_string())
            }
            MemoryParams::Get { key } => {
                let row: Option<(String,)> = sqlx::query_as(
                    "SELECT value FROM memory WHERE key = ?"
                )
                .bind(&key)
                .fetch_optional(self.store.pool())
                .await
                .map_err(|e| ToolError::Execution(format!("Memory get error: {}", e)))?;
                match row {
                    Some((value,)) => Ok(json!({ "key": key, "value": value }).to_string()),
                    None => Ok(json!({ "key": key, "value": null }).to_string()),
                }
            }
            MemoryParams::Search { query, limit } => {
                let limit = limit.unwrap_or(5);
                let pattern = format!("%{}%", query);
                let rows: Vec<(String, String)> = sqlx::query_as(
                    "SELECT key, value FROM memory WHERE value LIKE ? LIMIT ?"
                )
                .bind(&pattern)
                .bind(limit as i64)
                .fetch_all(self.store.pool())
                .await
                .map_err(|e| ToolError::Execution(format!("Memory search error: {}", e)))?;
                let results: Vec<_> = rows.into_iter().map(|(k, v)| json!({"key": k, "value": v})).collect();
                Ok(json!({ "results": results }).to_string())
            }
        }
    }
}
```

### 步骤 4: 添加 pool 访问方法到 SqliteSessionStore

在 `crates/hermes-memory/src/sqlite_store.rs` 的 `SqliteSessionStore` impl 块中添加：

```rust
pub fn pool(&self) -> &SqlitePool {
    &self.pool
}
```

### 步骤 5: 更新 lib.rs 中的 register_extended_tools

修改函数签名以接收 `session_store: Arc<SqliteSessionStore>` 参数。

### 步骤 6: 运行测试

Run: `cargo check -p hermes-tools-extended -p hermes-memory`
Expected: 编译通过

### 步骤 7: 提交

```bash
git add crates/hermes-tools-extended/src/memory.rs crates/hermes-memory/src/sqlite_store.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(tools-extended): add MemoryTool for cross-session KV storage"
```

---

## Task 4: 集成验证

**Files:**
- Modify: `crates/hermes-tools-extended/src/lib.rs`

### 步骤 1: 确认 register_extended_tools 签名正确

检查调用方（hermes-core 或 hermes-cli）是否需要更新以传入新增参数。

### 步骤 2: 运行完整编译和测试

Run: `cargo check --all`
Run: `cargo test -p hermes-tools-extended -p hermes-memory -p hermes-core`

### 步骤 3: 提交

```bash
git add -A
git commit -m "chore: integrate VisionTool and MemoryTool"
```

---

## 验收清单

### VisionTool
- [ ] `cargo test -p hermes-tools-extended test_vision` PASS
- [ ] 支持本地文件路径（base64 编码）
- [ ] 支持 URL 直接传入
- [ ] 错误处理正确

### MemoryTool
- [ ] `memory_set` 正确持久化（INSERT OR REPLACE）
- [ ] `memory_get` 正确读取，不存在返回 null
- [ ] `memory_search` 子串匹配正确
- [ ] memory 表 schema 正确

### 集成
- [ ] `cargo check --all` PASS
- [ ] `cargo test -p hermes-tools-extended` PASS

---

## 关键类型对照

| 类型/方法 | 定义位置 |
|-----------|---------|
| `Content::Image { url, detail }` | `hermes-core/src/types.rs:43` |
| `Message::user(content)` | `hermes-core/src/types.rs` |
| `LlmProvider::chat()` | `hermes-core/src/traits/llm_provider.rs` |
| `SqliteSessionStore::new()` | `hermes-memory/src/sqlite_store.rs:111` |
| `SqliteSessionStore::pool()` | 需新增（Task 3 Step 4） |
| `Tool` trait | `hermes-tool-registry/src/lib.rs` |
| `register_extended_tools()` | `hermes-tools-extended/src/lib.rs:28` |
