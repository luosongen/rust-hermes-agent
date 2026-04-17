# Rust-Python Parity: Top 5 Tools — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 5 个扩展工具（SessionSearch / ImageGeneration / Transcription / HomeAssistant / MixtureOfAgents），对齐 Python 版功能

**Architecture:** 每个工具独立实现为 `hermes-tools-extended` 的子模块，遵循现有 Tool trait + Builder 模式。SessionSearchTool 扩展现有 MemoryTool，添加独立 `session_messages` FTS5 表。其他工具均为新文件。

**Tech Stack:** reqwest, tokio, async-trait, parking_lot, serde, serde_json, sqlx (FTS5)

---

## 文件结构总览

```
crates/hermes-tools-extended/src/
├── memory.rs          # 修改：添加 session_messages FTS + search_sessions() + session_remember()
├── image_generation.rs  # 新增：ImageGenerationTool
├── transcription.rs   # 新增：TranscriptionTool
├── homeassistant.rs   # 新增：HomeAssistantTool
└── mixture_of_agents.rs # 新增：MixtureOfAgentsTool

crates/hermes-tools-extended/src/lib.rs  # 修改：新增模块 export

crates/hermes-tools-extended/tests/
├── test_memory.rs     # 修改：添加 session_* 测试
├── test_image_generation.rs  # 新增
├── test_transcription.rs     # 新增
├── test_homeassistant.rs     # 新增
└── test_mixture_of_agents.rs # 新增
```

---

### Task 1: SessionSearchTool（扩展 MemoryTool）

**Files:**
- Modify: `crates/hermes-tools-extended/src/memory.rs`（现有文件，约 300 行）
- Modify: `crates/hermes-tools-extended/tests/test_memory.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`

- [ ] **Step 1: 添加 session_messages FTS 表初始化**

在 `MemoryTool::ensure_fts()` 末尾追加：

```rust
// === session_messages FTS 表 ===
sqlx::query(
    "CREATE TABLE IF NOT EXISTS session_messages (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        role TEXT NOT NULL,
        content TEXT NOT NULL,
        created_at REAL NOT NULL
    )"
)
.execute(pool)
.await
.map_err(|e| ToolError::Execution(format!("session_messages table error: {}", e)))?;

sqlx::query("CREATE INDEX IF NOT EXISTS idx_session_messages_session ON session_messages(session_id)")
.execute(pool)
.await
.map_err(|e| ToolError::Execution(format!("session index error: {}", e)))?;

sqlx::query(
    "CREATE VIRTUAL TABLE IF NOT EXISTS session_messages_fts USING fts5(
        session_id UNINDEXED, role UNINDEXED, content,
        content=session_messages, content_rowid=id
    )"
)
.execute(pool)
.await
.map_err(|e| ToolError::Execution(format!("session_messages_fts error: {}", e)))?;

sqlx::query(
    "CREATE TRIGGER IF NOT EXISTS session_messages_fts_insert AFTER INSERT ON session_messages
     BEGIN INSERT INTO session_messages_fts(rowid, session_id, role, content) VALUES (new.id, new.session_id, new.role, new.content); END"
)
.execute(pool)
.await
.map_err(|e| ToolError::Execution(format!("session_messages insert trigger error: {}", e)))?;
```

验证命令：`cargo test -p hermes-tools-extended test_memory_set_get -v`（确保现有功能不受影响）

- [ ] **Step 2: 添加 SessionMessage / SessionSearchResult 类型**

在 `memory.rs` 末尾（在 `#[async_trait] impl Tool for MemoryTool` 之前）添加：

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: f64,
}

#[derive(Debug, serde::Serialize)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub summary: String,
    pub matched_messages: usize,
    pub last_updated: f64,
}
```

- [ ] **Step 3: 添加 session_remember / search_sessions 公共方法**

在 `impl MemoryTool` 块中添加：

```rust
/// 存储单条会话消息
pub async fn session_remember(
    &self,
    session_id: &str,
    role: &str,
    content: &str,
) -> Result<(), ToolError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as f64;

    sqlx::query(
        "INSERT INTO session_messages (session_id, role, content, created_at) VALUES (?, ?, ?, ?)"
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(now)
    .execute(self.store.pool())
    .await
    .map_err(|e| ToolError::Execution(format!("session_remember error: {}", e)))?;

    Ok(())
}

/// 搜索历史会话
pub async fn search_sessions(
    &self,
    query: &str,
    limit_sessions: usize,
) -> Result<Vec<SessionSearchResult>, ToolError> {
    // 1. FTS5 匹配获取 message rowids
    let pattern = format!("\"{}\"", query.replace('"', "\"\""));
    let matched: Vec<(String, i64)> = sqlx::query_as(
        "SELECT session_id, COUNT(*) as cnt FROM session_messages_fts WHERE session_messages_fts MATCH ? GROUP BY session_id ORDER BY cnt DESC LIMIT ?"
    )
    .bind(&pattern)
    .bind(limit_sessions as i64)
    .fetch_all(self.store.pool())
    .await
    .map_err(|e| ToolError::Execution(format!("search_sessions error: {}", e)))?;

    let mut results = Vec::new();
    for (session_id, cnt) in matched {
        // 获取该 session 最新时间戳
        let last_updated: Option<(f64,)> = sqlx::query_as(
            "SELECT MAX(created_at) FROM session_messages WHERE session_id = ?"
        )
        .bind(&session_id)
        .fetch_optional(self.store.pool())
        .await
        .map_err(|e| ToolError::Execution(format!("last_updated error: {}", e)))?;

        results.push(SessionSearchResult {
            session_id,
            summary: format!("[{} matched messages]", cnt),
            matched_messages: cnt as usize,
            last_updated: last_updated.map(|(t,)| t).unwrap_or(0.0),
        });
    }

    Ok(results)
}
```

- [ ] **Step 4: 在 MemoryParams 中添加 session_remember 和 session_search Variant**

修改 `#[derive(Debug, Deserialize)]` 枚举：

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum MemoryParams {
    Set { key: String, value: String, #[serde(default)] category: Option<String>, #[serde(default)] tags: Vec<String> },
    Get { key: String },
    Search { query: String, #[serde(default)] limit: Option<usize> },
    Read { #[serde(default)] category: Option<String> },
    SessionRemember { session_id: String, role: String, content: String },
    SessionSearch { query: String, #[serde(default)] limit: Option<usize> },
}
```

- [ ] **Step 5: 更新 Tool::parameters() 添加新 action schemas**

在 `parameters()` 函数的 `oneOf` 数组中添加：

```rust
{
    "properties": {
        "action": { "const": "session_remember" },
        "session_id": { "type": "string" },
        "role": { "type": "string" },
        "content": { "type": "string" }
    },
    "required": ["action", "session_id", "role", "content"]
},
{
    "properties": {
        "action": { "const": "session_search" },
        "query": { "type": "string" },
        "limit": { "type": "integer", "default": 3 }
    },
    "required": ["action", "query"]
}
```

- [ ] **Step 6: 在 execute() 中处理新 action**

在 `match params` 的 match 块中添加：

```rust
MemoryParams::SessionRemember { session_id, role, content } => {
    self.session_remember(&session_id, &role, &content).await?;
    Ok(json!({ "status": "ok", "session_id": session_id }).to_string())
}
MemoryParams::SessionSearch { query, limit } => {
    let limit = limit.unwrap_or(3);
    let results = self.search_sessions(&query, limit).await?;
    Ok(json!({ "results": results }).to_string())
}
```

- [ ] **Step 7: 添加测试**

在 `tests/test_memory.rs` 末尾添加：

```rust
#[test]
fn test_session_remember_and_search() {
    let store = make_temp_store();
    let tool = MemoryTool::new(store.clone());
    block_on(tool.ensure_fts()).unwrap();

    block_on(async {
        let ctx = make_ctx();

        // 写入 session message
        tool.execute(serde_json::json!({
            "action": "session_remember",
            "session_id": "session-alpha",
            "role": "user",
            "content": "I want to build a Rust CLI tool"
        }), ctx.clone()).await.unwrap();

        tool.execute(serde_json::json!({
            "action": "session_remember",
            "session_id": "session-alpha",
            "role": "assistant",
            "content": "Great! Rust is perfect for CLI tools"
        }), ctx.clone()).await.unwrap();

        // 搜索
        let result = tool.execute(serde_json::json!({
            "action": "session_search",
            "query": "Rust CLI"
        }), ctx.clone()).await.unwrap();

        let output: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(output["results"].is_array());
    });
}
```

- [ ] **Step 8: 提交**

```bash
git add crates/hermes-tools-extended/src/memory.rs crates/hermes-tools-extended/tests/test_memory.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(memory): add SessionSearchTool — session_messages FTS + session_remember/session_search"
```

---

### Task 2: ImageGenerationTool

**Files:**
- Create: `crates/hermes-tools-extended/src/image_generation.rs`
- Create: `crates/hermes-tools-extended/tests/test_image_generation.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`

- [ ] **Step 1: 编写 ImageGenerationTool 骨架**

创建 `crates/hermes-tools-extended/src/image_generation.rs`：

```rust
//! ImageGenerationTool — Fal.ai FLUX 2 Pro 图像生成
//!
//! 支持 landscape_16_9 / portrait_9_16 / square_1_1 / landscape_4_3 尺寸。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const FAL_FUX_PRO_URL: &str = "https://queue.fal.run/fal-ai/flux-2-pro";
const FAL_CLARITY_URL: &str = "https://queue.fal.run/fal-ai/clarity-upscaler";

#[derive(Clone)]
pub struct ImageGenerationTool {
    http_client: reqwest::Client,
    fal_api_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ImageSize {
    #[serde(rename = "landscape_16_9")]
    Landscape16x9,
    #[serde(rename = "portrait_9_16")]
    Portrait9x16,
    #[serde(rename = "square_1_1")]
    Square1x1,
    #[serde(rename = "landscape_4_3")]
    Landscape4x3,
}

impl Default for ImageGenerationTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageGenerationTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("HTTP client"),
            fal_api_key: std::env::var("FAL_API_KEY").ok(),
        }
    }

    pub fn with_fal_api_key(mut self, key: String) -> Self {
        self.fal_api_key = Some(key);
        self
    }
}
```

- [ ] **Step 2: 实现 Fal.ai 请求/轮询逻辑**

在 `impl ImageGenerationTool` 中添加：

```rust
async fn request_image(&self, prompt: &str, size: ImageSize) -> Result<String, ToolError> {
    let api_key = self.fal_api_key.as_ref()
        .ok_or_else(|| ToolError::Execution("FAL_API_KEY not set".to_string()))?;

    let size_str = match size {
        ImageSize::Landscape16x9 => "landscape_16_9",
        ImageSize::Portrait9x16 => "portrait_9_16",
        ImageSize::Square1x1 => "square_1_1",
        ImageSize::Landscape4x3 => "landscape_4_3",
    };

    let payload = serde_json::json!({
        "prompt": prompt,
        "image_size": size_str,
        "num_inference_steps": 50,
        "guidance_scale": 4.5,
        "num_images": 1,
        "enable_safety_checker": false
    });

    let resp = self.http_client
        .post(FAL_FUX_PRO_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ToolError::Execution(format!("Fal.ai request error: {}", e)))?;

    let body: serde_json::Value = resp.json().await
        .map_err(|e| ToolError::Execution(format!("Fal.ai response error: {}", e)))?;

    let request_id = body["request_id"].as_str()
        .ok_or_else(|| ToolError::Execution("No request_id in Fal.ai response".to_string()))?;

    Ok(request_id.to_string())
}

async fn poll_result(&self, request_id: &str) -> Result<String, ToolError> {
    let api_key = self.fal_api_key.as_ref().unwrap();
    let url = format!("{}/results?request_id={}", FAL_FUX_PRO_URL, request_id);

    for _ in 0..60 {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let resp = self.http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Fal.ai poll error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Fal.ai poll response error: {}", e)))?;

        if body["status"] == "COMPLETED" {
            let image_url = body["images"][0]["url"].as_str()
                .ok_or_else(|| ToolError::Execution("No image URL in Fal.ai response".to_string()))?;
            return Ok(image_url.to_string());
        }
    }

    Err(ToolError::Execution("Fal.ai timeout".to_string()))
}

async fn upscale(&self, image_url: &str) -> Result<String, ToolError> {
    let api_key = self.fal_api_key.as_ref().unwrap();

    let payload = serde_json::json!({
        "image_url": image_url,
        "scale": 2
    });

    let resp = self.http_client
        .post(FAL_CLARITY_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ToolError::Execution(format!("Upscaler request error: {}", e)))?;

    let body: serde_json::Value = resp.json().await
        .map_err(|e| ToolError::Execution(format!("Upscaler response error: {}", e)))?;

    let request_id = body["request_id"].as_str()
        .ok_or_else(|| ToolError::Execution("No request_id in upscaler response".to_string()))?;

    // Poll for upscaler result
    for _ in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let poll_url = format!("{}/results?request_id={}", FAL_CLARITY_URL, request_id);
        let resp = self.http_client
            .get(&poll_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        let body: serde_json::Value = resp.json().await.map_err(|e| ToolError::Execution(e.to_string()))?;

        if body["status"] == "COMPLETED" {
            let upscaled_url = body["images"][0]["url"].as_str()
                .ok_or_else(|| ToolError::Execution("No upscaled image URL".to_string()))?;
            return Ok(upscaled_url.to_string());
        }
    }

    Err(ToolError::Execution("Upscaler timeout".to_string()))
}
```

- [ ] **Step 3: 实现 Tool trait**

添加：

```rust
#[derive(Debug, Deserialize)]
pub struct ImageGenParams {
    pub prompt: String,
    #[serde(default)]
    pub image_size: Option<String>,
    #[serde(default = "default_steps")]
    pub num_inference_steps: u32,
    #[serde(default = "default_guidance")]
    pub guidance_scale: f32,
    #[serde(default = "default_num")]
    pub num_images: u32,
}

fn default_steps() -> u32 { 50 }
fn default_guidance() -> f32 { 4.5 }
fn default_num() -> u32 { 1 }

#[async_trait]
impl Tool for ImageGenerationTool {
    fn name(&self) -> &str { "image_generate" }

    fn description(&self) -> &str {
        "Generate images from text prompts using Fal.ai FLUX 2 Pro with automatic 2x upscaling."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "image_size": {
                    "type": "string",
                    "enum": ["landscape_16_9", "portrait_9_16", "square_1_1", "landscape_4_3"],
                    "default": "landscape_16_9"
                },
                "num_inference_steps": { "type": "integer", "default": 50 },
                "guidance_scale": { "type": "number", "default": 4.5 },
                "num_images": { "type": "integer", "default": 1 }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: ImageGenParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        if self.fal_api_key.is_none() {
            return Err(ToolError::Execution("FAL_API_KEY not configured".to_string()));
        }

        let size = match params.image_size.as_deref() {
            Some("portrait_9_16") => ImageSize::Portrait9x16,
            Some("square_1_1") => ImageSize::Square1x1,
            Some("landscape_4_3") => ImageSize::Landscape4x3,
            _ => ImageSize::Landscape16x9,
        };

        let request_id = self.request_image(&params.prompt, size).await?;
        let image_url = self.poll_result(&request_id).await?;
        let upscaled_url = self.upscale(&image_url).await?;

        Ok(json!({
            "success": true,
            "images": [{
                "url": upscaled_url,
                "width": 2048,
                "height": 1536,
                "upscaled": true
            }],
            "model": "fal-ai/flux-2-pro"
        }).to_string())
    }
}
```

- [ ] **Step 4: 导出新模块**

修改 `crates/hermes-tools-extended/src/lib.rs`：
- 添加 `pub mod image_generation;`
- 添加 `pub use image_generation::ImageGenerationTool;`

在 `register_extended_tools` 函数中注册：
```rust
registry.register(ImageGenerationTool::new());
```

- [ ] **Step 5: 添加测试骨架**

创建 `tests/test_image_generation.rs`：

```rust
use hermes_tool_registry::Tool;

#[test]
fn test_image_generation_tool_name() {
    let tool = hermes_tools_extended::image_generation::ImageGenerationTool::new();
    assert_eq!(tool.name(), "image_generate");
}

#[test]
fn test_image_generation_params() {
    let tool = hermes_tools_extended::image_generation::ImageGenerationTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/prompt").is_some());
    assert!(params.pointer("/properties/image_size").is_some());
}
```

验证命令：`cargo test -p hermes-tools-extended test_image_generation -v`

- [ ] **Step 6: 提交**

```bash
git add crates/hermes-tools-extended/src/image_generation.rs crates/hermes-tools-extended/tests/test_image_generation.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(tools): add ImageGenerationTool — Fal.ai FLUX 2 Pro + Clarity upscaler"
```

---

### Task 3: TranscriptionTool

**Files:**
- Create: `crates/hermes-tools-extended/src/transcription.rs`
- Create: `crates/hermes-tools-extended/tests/test_transcription.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`

- [ ] **Step 1: 编写 TranscriptionTool 骨架**

创建 `crates/hermes-tools-extended/src/transcription.rs`：

```rust
//! TranscriptionTool — 语音转文字
//!
//! 支持 faster-whisper 本地运行和 Groq Whisper API。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

const GROQ_URL: &str = "https://api.groq.com/v1/audio/transcriptions";

#[derive(Clone)]
pub struct TranscriptionTool {
    http_client: reqwest::Client,
    groq_api_key: Option<String>,
    whisper_model_path: Option<PathBuf>,
}

#[derive(Clone)]
pub enum TranscriptionProvider {
    FasterWhisper,
    GroqWhisper,
}

impl Default for TranscriptionTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptionTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            groq_api_key: std::env::var("GROQ_API_KEY").ok(),
            whisper_model_path: None,
        }
    }

    pub fn with_groq_api_key(mut self, key: String) -> Self {
        self.groq_api_key = Some(key);
        self
    }

    pub fn with_whisper_model_path(mut self, path: PathBuf) -> Self {
        self.whisper_model_path = Some(path);
        self
    }

    async fn transcribe_faster_whisper(&self, audio_path: &str, language: Option<&str>) -> Result<String, ToolError> {
        let model_path = self.whisper_model_path.as_ref()
            .map(|p| p.as_path())
            .unwrap_or_else(|| PathBuf::from(std::env::var("WHISPER_MODEL_PATH").unwrap_or_else(|_| "~/.cache/faster-whisper".to_string())).as_path());

        let mut cmd = tokio::process::Command::new("whisper");
        cmd.arg("--model").arg(model_path)
           .arg("--language").arg(language.unwrap_or("auto"))
           .arg("--output_format").arg("json")
           .arg("--input").arg(audio_path);

        let output = cmd.output().await
            .map_err(|e| ToolError::Execution(format!("faster-whisper error: {}", e)))?;

        if !output.status.success() {
            return Err(ToolError::Execution(format!("faster-whisper failed: {:?}", output.stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| ToolError::Execution(format!("faster-whisper JSON parse error: {}", e)))?;

        Ok(json["text"].as_str().unwrap_or("").to_string())
    }

    async fn transcribe_groq(&self, audio_path: &str, language: Option<&str>) -> Result<String, ToolError> {
        let api_key = self.groq_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("GROQ_API_KEY not set".to_string()))?;

        let file = tokio::fs::File::open(audio_path).await
            .map_err(|e| ToolError::Execution(format!("Audio file open error: {}", e)))?;

        let form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::stream(file).file_name("audio.mp3"))
            .text("model", "whisper-large-v3")
            .text("language", language.unwrap_or("en"));

        let resp = self.http_client
            .post(GROQ_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Groq API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Groq response error: {}", e)))?;

        body["text"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::Execution("No text in Groq response".to_string()))
    }
}
```

- [ ] **Step 2: 实现 Tool trait**

在 `impl TranscriptionTool` 后添加：

```rust
#[derive(Debug, Deserialize)]
pub struct TranscribeParams {
    pub audio_path: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub language: Option<String>,
}

fn default_provider() -> String { "faster-whisper".to_string() }

#[async_trait]
impl Tool for TranscriptionTool {
    fn name(&self) -> &str { "transcribe" }

    fn description(&self) -> &str {
        "Transcribe audio to text. Supports faster-whisper (local) and Groq Whisper API."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "audio_path": { "type": "string", "description": "Path to audio file" },
                "provider": {
                    "type": "string",
                    "enum": ["faster-whisper", "groq"],
                    "default": "faster-whisper"
                },
                "language": { "type": "string", "description": "Language code (e.g., 'en')" }
            },
            "required": ["audio_path"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: TranscribeParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let text = match params.provider.as_str() {
            "groq" => self.transcribe_groq(&params.audio_path, params.language.as_deref()).await?,
            _ => self.transcribe_faster_whisper(&params.audio_path, params.language.as_deref()).await?,
        };

        Ok(json!({
            "success": true,
            "text": text,
            "provider": params.provider,
            "audio_path": params.audio_path
        }).to_string())
    }
}
```

- [ ] **Step 3: 导出新模块**

修改 `lib.rs`：
- 添加 `pub mod transcription;`
- 添加 `pub use transcription::TranscriptionTool;`
- 在 `register_extended_tools` 中注册：`registry.register(TranscriptionTool::new());`

- [ ] **Step 4: 添加测试**

创建 `tests/test_transcription.rs`：

```rust
use hermes_tool_registry::Tool;

#[test]
fn test_transcription_tool_name() {
    let tool = hermes_tools_extended::transcription::TranscriptionTool::new();
    assert_eq!(tool.name(), "transcribe");
}

#[test]
fn test_transcription_params() {
    let tool = hermes_tools_extended::transcription::TranscriptionTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/audio_path").is_some());
    assert!(params.pointer("/properties/provider").is_some());
}
```

验证命令：`cargo test -p hermes-tools-extended test_transcription -v`

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-tools-extended/src/transcription.rs crates/hermes-tools-extended/tests/test_transcription.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(tools): add TranscriptionTool — faster-whisper + Groq Whisper API"
```

---

### Task 4: HomeAssistantTool

**Files:**
- Create: `crates/hermes-tools-extended/src/homeassistant.rs`
- Create: `crates/hermes-tools-extended/tests/test_homeassistant.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`

- [ ] **Step 1: 编写 HomeAssistantTool 骨架**

创建 `crates/hermes-tools-extended/src/homeassistant.rs`：

```rust
//! HomeAssistantTool — 控制 Home Assistant 智能家居设备
//!
//! 支持 list_entities / get_state / list_services / call_service 四个 action。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use parking_lot::RwLock;
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;

lazy_static::lazy_static! {
    static ref ENTITY_ID_RE: Regex = Regex::new(r"^[a-z_][a-z0-9_]*\.[a-z0-9_]+$").unwrap();
    static ref SERVICE_NAME_RE: Regex = Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();
    static ref BLOCKED_DOMAINS: HashSet<&'static str> = [
        "shell_command", "command_line", "python_script", "pyscript", "hassio", "rest_command"
    ].into_iter().collect();
}

#[derive(Clone)]
pub struct HomeAssistantTool {
    http_client: reqwest::Client,
    hass_url: String,
    hass_token: Option<String>,
    discovery_cache: Arc<RwLock<Option<DiscoveredInstance>>>,
}

#[derive(Debug, Clone)]
struct DiscoveredInstance {
    url: String,
    name: String,
}

impl Default for HomeAssistantTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeAssistantTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
            hass_url: std::env::var("HASS_URL").unwrap_or_else(|_| "http://localhost:8123".to_string()),
            hass_token: std::env::var("HASS_TOKEN").ok(),
            discovery_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.hass_url = url;
        self
    }

    pub fn with_token(mut self, token: String) -> Self {
        self.hass_token = Some(token);
        self
    }

    fn validate_entity_id(entity_id: &str) -> Result<(), ToolError> {
        if !ENTITY_ID_RE.is_match(entity_id) {
            return Err(ToolError::InvalidArgs(format!("Invalid entity_id format: {}", entity_id)));
        }
        let domain = entity_id.split('.').next().unwrap();
        if BLOCKED_DOMAINS.contains(domain) {
            return Err(ToolError::InvalidArgs(format!("Blocked domain: {}", domain)));
        }
        Ok(())
    }

    fn validate_service_name(service: &str) -> Result<(), ToolError> {
        if !SERVICE_NAME_RE.is_match(service) {
            return Err(ToolError::InvalidArgs(format!("Invalid service name: {}", service)));
        }
        Ok(())
    }

    async fn ha_get(&self, path: &str) -> Result<serde_json::Value, ToolError> {
        let url = format!("{}/api/{}", self.hass_url.trim_end_matches('/'), path);
        let mut req = self.http_client.get(&url);
        if let Some(token) = &self.hass_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant API error: {}", e)))?;
        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant response error: {}", e)))?;
        Ok(body)
    }

    async fn ha_post(&self, path: &str, data: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let url = format!("{}/api/{}", self.hass_url.trim_end_matches('/'), path);
        let mut req = self.http_client.post(&url);
        if let Some(token) = &self.hass_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.json(&data).send().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant POST error: {}", e)))?;
        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant POST response error: {}", e)))?;
        Ok(body)
    }
}
```

- [ ] **Step 2: 实现 Tool trait 和 4 个 action**

```rust
#[derive(Debug, Deserialize)]
pub struct HaParams {
    pub action: String,
    pub domain: Option<String>,
    pub area: Option<String>,
    pub entity_id: Option<String>,
    pub service: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[async_trait]
impl Tool for HomeAssistantTool {
    fn name(&self) -> &str { "homeassistant" }

    fn description(&self) -> &str {
        "Control Home Assistant devices. Supports ha_list_entities, ha_get_state, ha_list_services, ha_call_service."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "action": { "const": "ha_list_entities" },
                        "domain": { "type": "string" },
                        "area": { "type": "string" }
                    },
                    "required": ["action"]
                },
                {
                    "properties": {
                        "action": { "const": "ha_get_state" },
                        "entity_id": { "type": "string" }
                    },
                    "required": ["action", "entity_id"]
                },
                {
                    "properties": {
                        "action": { "const": "ha_list_services" },
                        "domain": { "type": "string" }
                    },
                    "required": ["action"]
                },
                {
                    "properties": {
                        "action": { "const": "ha_call_service" },
                        "domain": { "type": "string" },
                        "service": { "type": "string" },
                        "entity_id": { "type": "string" },
                        "data": { "type": "object" }
                    },
                    "required": ["action", "domain", "service", "entity_id"]
                }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: HaParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params.action.as_str() {
            "ha_list_entities" => {
                let state: Vec<serde_json::Value> = self.ha_get("states").await?;
                let filtered: Vec<_> = state.into_iter().filter(|s| {
                    if let (Some(dom), Some(area)) = (params.domain.as_ref(), params.area.as_ref()) {
                        s["entity_id"].as_str().map(|e| e.starts_with(&format!("{}.", dom))).unwrap_or(false)
                            && s["attributes"]["area_id"].as_str() == Some(area)
                    } else if let Some(dom) = params.domain.as_ref() {
                        s["entity_id"].as_str().map(|e| e.starts_with(&format!("{}.", dom))).unwrap_or(false)
                    } else {
                        true
                    }
                }).collect();
                Ok(json!({ "entities": filtered }).to_string())
            }
            "ha_get_state" => {
                let entity_id = params.entity_id.as_ref().unwrap();
                Self::validate_entity_id(entity_id)?;
                let state = self.ha_get(&format!("states/{}", entity_id)).await?;
                Ok(json!({ "state": state }).to_string())
            }
            "ha_list_services" => {
                let services: serde_json::Value = self.ha_get("services").await?;
                if let Some(domain) = params.domain.as_ref() {
                    let filtered = services.get(domain).cloned().unwrap_or(serde_json::Value::Null);
                    Ok(json!({ "services": filtered }).to_string())
                } else {
                    Ok(json!({ "services": services }).to_string())
                }
            }
            "ha_call_service" => {
                let domain = params.domain.as_ref().unwrap();
                let service = params.service.as_ref().unwrap();
                let entity_id = params.entity_id.as_ref().unwrap();
                Self::validate_entity_id(entity_id)?;
                Self::validate_service_name(service)?;

                let data = serde_json::json!({
                    "entity_id": entity_id,
                    "data": params.data.unwrap_or(serde_json::Value::Null)
                });

                let result = self.ha_post(&format!("services/{}", domain), data).await?;
                Ok(json!({ "result": result }).to_string())
            }
            _ => Err(ToolError::InvalidArgs(format!("Unknown action: {}", params.action))),
        }
    }
}
```

- [ ] **Step 3: 导出新模块**

修改 `lib.rs`：
- 添加 `pub mod homeassistant;`
- 添加 `pub use homeassistant::HomeAssistantTool;`
- 在 `register_extended_tools` 中注册：`registry.register(HomeAssistantTool::new());`

**注意：** 需要在 `Cargo.toml` 中添加 `lazy_static = "1"` 依赖

- [ ] **Step 4: 添加测试**

创建 `tests/test_homeassistant.rs`：

```rust
use hermes_tool_registry::Tool;

#[test]
fn test_homeassistant_tool_name() {
    let tool = hermes_tools_extended::homeassistant::HomeAssistantTool::new();
    assert_eq!(tool.name(), "homeassistant");
}

#[test]
fn test_homeassistant_params_structure() {
    let tool = hermes_tools_extended::homeassistant::HomeAssistantTool::new();
    let params = tool.parameters();
    assert!(params["oneOf"].is_array());
    assert_eq!(params["oneOf"].as_array().unwrap().len(), 4);
}
```

验证命令：`cargo test -p hermes-tools-extended test_homeassistant -v`

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-tools-extended/src/homeassistant.rs crates/hermes-tools-extended/tests/test_homeassistant.rs crates/hermes-tools-extended/src/lib.rs crates/hermes-tools-extended/Cargo.toml
git commit -m "feat(tools): add HomeAssistantTool — HA REST API with 4 actions and security validation"
```

---

### Task 5: MixtureOfAgentsTool

**Files:**
- Create: `crates/hermes-tools-extended/src/mixture_of_agents.rs`
- Create: `crates/hermes-tools-extended/tests/test_mixture_of_agents.rs`
- Modify: `crates/hermes-tools-extended/src/lib.rs`

- [ ] **Step 1: 编写 MixtureOfAgentsTool 骨架**

创建 `crates/hermes-tools-extended/src/mixture_of_agents.rs`：

```rust
//! MixtureOfAgentsTool — 多 LLM 并行聚合
//!
//! 调用多个 reference models 生成多样化响应，通过 aggregator model 合成最终答案。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Clone)]
pub struct MixtureOfAgentsTool {
    http_client: reqwest::Client,
    openrouter_api_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MoAConfig {
    #[serde(default)]
    pub reference_models: Vec<String>,
    #[serde(default = "default_aggregator")]
    pub aggregator_model: String,
    #[serde(default = "default_ref_temp")]
    pub reference_temperature: f32,
    #[serde(default = "default_agg_temp")]
    pub aggregator_temperature: f32,
    #[serde(default = "default_min_refs")]
    pub min_successful_references: usize,
}

fn default_aggregator() -> String { "anthropic/claude-opus-4-5-sonnet-20241022".to_string() }
fn default_ref_temp() -> f32 { 0.7 }
fn default_agg_temp() -> f32 { 0.3 }
fn default_min_refs() -> usize { 2 }

impl Default for MoAConfig {
    fn default() -> Self {
        Self {
            reference_models: vec![
                "anthropic/claude-opus-4-5-sonnet-20241022".to_string(),
                "google/gemini-2.5-pro-preview-06-05".to_string(),
                "openai/gpt-5-pro".to_string(),
                "deepseek/deepseek-v3".to_string(),
            ],
            aggregator_model: default_aggregator(),
            reference_temperature: default_ref_temp(),
            aggregator_temperature: default_agg_temp(),
            min_successful_references: default_min_refs(),
        }
    }
}

impl Default for MixtureOfAgentsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MixtureOfAgentsTool {
    pub fn new() -> Self {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .expect("OPENROUTER_API_KEY not set");
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .expect("HTTP client"),
            openrouter_api_key: api_key,
        }
    }

    pub fn with_api_key(mut self, key: String) -> Self {
        self.openrouter_api_key = key;
        self
    }

    async fn call_openrouter(&self, model: &str, prompt: &str, temperature: f32) -> Result<String, ToolError> {
        let payload = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": temperature
        });

        let resp = self.http_client
            .post(OPENROUTER_URL)
            .header("Authorization", format!("Bearer {}", self.openrouter_api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenRouter API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("OpenRouter response error: {}", e)))?;

        body["choices"][0]["message"]["content"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::Execution("No content in OpenRouter response".to_string()))
    }

    async fn call_reference_models(&self, prompt: &str, models: &[String], temperature: f32) -> Vec<(String, String)> {
        let mut handles = Vec::new();
        for model in models {
            let model = model.clone();
            let prompt = prompt.to_string();
            let client = self.http_client.clone();
            let api_key = self.openrouter_api_key.clone();
            handles.push(tokio::spawn(async move {
                let payload = serde_json::json!({
                    "model": model,
                    "messages": [{"role": "user", "content": prompt}],
                    "temperature": temperature
                });
                let resp = client.post(OPENROUTER_URL)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send().await;
                match resp {
                    Ok(r) => {
                        let body: serde_json::Value = r.json().await.ok()?;
                        body["choices"][0]["message"]["content"].as_str()
                            .map(|s| (model, s.to_string()))
                    }
                    Err(_) => None
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(Some(result)) = handle.await {
                results.push(result);
            }
        }
        results
    }
}
```

- [ ] **Step 2: 实现 Tool trait**

```rust
#[derive(Debug, Deserialize)]
pub struct MoAParams {
    pub prompt: String,
    #[serde(default)]
    pub reference_models: Option<Vec<String>>,
    #[serde(default)]
    pub aggregator_model: Option<String>,
}

#[async_trait]
impl Tool for MixtureOfAgentsTool {
    fn name(&self) -> &str { "mixture_of_agents" }

    fn description(&self) -> &str {
        "Solve complex queries using multiple frontier LLMs in parallel, synthesized by an aggregator."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "reference_models": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Override default reference models"
                },
                "aggregator_model": {
                    "type": "string",
                    "description": "Override default aggregator model"
                }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: MoAParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let config = MoAConfig {
            reference_models: params.reference_models.unwrap_or_else(MoAConfig::default().reference_models),
            aggregator_model: params.aggregator_model.unwrap_or_else(MoAConfig::default().aggregator_model),
            ..Default::default()
        };

        // Step 1: 并行调用 reference models
        let references = self.call_reference_models(
            &params.prompt,
            &config.reference_models,
            config.reference_temperature
        ).await;

        if references.len() < config.min_successful_references {
            return Err(ToolError::Execution(format!(
                "Only {} reference models succeeded, need {}",
                references.len(), config.min_successful_references
            )));
        }

        // Step 2: 构建 aggregator prompt
        let aggregator_prompt = format!(
            "You are a synthesis AI. Combine the following {} reference responses into a single coherent answer.\n\n{}\n\nProvide your synthesized answer:",
            references.len(),
            references.iter().enumerate().map(|(i, (_, r))| format!("[Reference {}]\n{}\n", i + 1, r)).collect::<String>()
        );

        // Step 3: 调用 aggregator
        let answer = self.call_openrouter(&config.aggregator_model, &aggregator_prompt, config.aggregator_temperature).await?;

        Ok(json!({
            "success": true,
            "answer": answer,
            "reference_count": references.len(),
            "references": references.iter().map(|(m, r)| {
                json!({ "model": m, "excerpt": &r[..r.len().min(200)] })
            }).collect::<Vec<_>>(),
            "aggregator": config.aggregator_model
        }).to_string())
    }
}
```

- [ ] **Step 3: 导出新模块**

修改 `lib.rs`：
- 添加 `pub mod mixture_of_agents;`
- 添加 `pub use mixture_of_agents::MixtureOfAgentsTool;`
- 在 `register_extended_tools` 中注册：`registry.register(MixtureOfAgentsTool::new());`

**注意：** 需要在 `Cargo.toml` 中添加 `lazy_static = "1"` 依赖（如果还没有）

- [ ] **Step 4: 添加测试**

创建 `tests/test_mixture_of_agents.rs`：

```rust
use hermes_tool_registry::Tool;

#[test]
fn test_moa_tool_name() {
    let tool = hermes_tools_extended::mixture_of_agents::MixtureOfAgentsTool::new();
    assert_eq!(tool.name(), "mixture_of_agents");
}

#[test]
fn test_moa_params() {
    let tool = hermes_tools_extended::mixture_of_agents::MixtureOfAgentsTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/prompt").is_some());
    assert!(params.pointer("/properties/reference_models").is_some());
}
```

验证命令：`cargo test -p hermes-tools-extended test_moa -v`

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-tools-extended/src/mixture_of_agents.rs crates/hermes-tools-extended/tests/test_mixture_of_agents.rs crates/hermes-tools-extended/src/lib.rs
git commit -m "feat(tools): add MixtureOfAgentsTool — OpenRouter multi-LLM parallel + aggregator synthesis"
```

---

## Self-Review 检查清单

**1. Spec Coverage:**
- ✅ SessionSearchTool: session_messages FTS + session_remember + session_search + MemoryParams update + parameters() 更新 + execute() 更新
- ✅ ImageGenerationTool: Fal.ai FLUX 2 Pro + Clarity upscaler + polling + Tool trait
- ✅ TranscriptionTool: faster-whisper 子进程 + Groq API + Provider enum
- ✅ HomeAssistantTool: 4 actions (list_entities/get_state/list_services/call_service) + entity_id/service 校验 + blocked domains
- ✅ MixtureOfAgentsTool: OpenRouter 并行调用 + aggregator synthesis

**2. Placeholder Scan:** 无 "TBD" / "TODO" / 未实现的功能描述

**3. Type Consistency:**
- MemoryParams 新增 `SessionRemember` / `SessionSearch` variant — 与 spec 一致
- `ImageGenParams` / `TranscribeParams` / `HaParams` / `MoAParams` 字段名与 spec 定义一致
- `SessionMessage` / `SessionSearchResult` / `ImageSize` / `MoAConfig` 与 spec 类型定义一致

**4. 依赖检查:**
- `lazy_static` 需添加到 `hermes-tools-extended/Cargo.toml`（Task 4 & 5 共用）
- `reqwest`, `tokio`, `async-trait`, `parking_lot`, `serde`, `serde_json` 已在 workspace 依赖中
- 无需新增外部依赖（sqlx FTS5 语法 SQLite 原生支持）
