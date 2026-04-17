# Rust-Python Parity: Top 5 Tools — Design Spec

> **Status:** Draft
> **Date:** 2026-04-16
> **Scope:** 5 new tools to extend Rust hermes-agent feature parity
> **Local-first:** All tools work without cloud dependencies where possible

---

## 概述

本文档定义 5 个缺失工具的设计，实现 Rust 版 hermes-agent 与 Python 版的功能对齐。

**设计原则：**
- 本地优先：核心功能不依赖外部云服务
- 云 API 可选：API key 配置后启用
- 增量实现：每个工具独立开发和测试
- 架构一致性：遵循 Rust 版的现有模式（Tool trait、Builder 模式等）

---

## 实现顺序

1. **SessionSearchTool** — 扩展 MemoryTool，搜索历史会话 FTS5 + LLM summarization
2. **ImageGenerationTool** — Fal.ai FLUX 2 Pro + 自动 2x upscaling
3. **TranscriptionTool** — faster-whisper 本地 + Groq Whisper API
4. **HomeAssistantTool** — HA REST API，auto-discovery + 安全验证
5. **MixtureOfAgentsTool** — 多 LLM 并行聚合，OpenRouter

---

## 1. SessionSearchTool (扩展 MemoryTool)

### 目标

扩展现有 MemoryTool，添加会话历史搜索功能。在 SQLite FTS5 中查找匹配的 session messages，按 session 分组，取 top N 个 session，用 LLM summarization 生成摘要。

### 核心类型

```rust
// MemoryTool 新增方法
impl MemoryTool {
    /// 搜索历史会话
    pub async fn search_sessions(
        &self,
        query: &str,
        limit_sessions: usize,  // 默认 3
        llm_provider: &Arc<dyn LlmProvider>,
    ) -> Result<Vec<SessionSearchResult>, ToolError>;

    /// 获取某 session 的 messages（用于截断后 summarization）
    async fn get_session_messages(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>, ToolError>;
}

#[derive(Debug, serde::Serialize)]
pub struct SessionSearchResult {
    pub session_id: String,
    pub summary: String,           // LLM 生成的摘要
    pub matched_messages: usize,  // 匹配的消息数
    pub last_updated: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    pub timestamp: f64,
}
```

### 工具接口

```json
{
  "name": "session_search",
  "description": "Search past session transcripts via FTS5 and summarize with LLM.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": { "type": "string" },
      "limit": { "type": "integer", "default": 3 }
    },
    "required": ["query"]
  }
}
```

### FTS5 Schema 扩展

```sql
-- 新增 session_messages 表（独立于 existing memory table）
CREATE TABLE IF NOT EXISTS session_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_session_messages_session ON session_messages(session_id);

CREATE VIRTUAL TABLE IF NOT EXISTS session_messages_fts USING fts5(
    session_id UNINDEXED,
    role UNINDEXED,
    content,
    content=session_messages,
    content_rowid=id
);

-- Triggers
CREATE TRIGGER IF NOT EXISTS session_messages_fts_insert AFTER INSERT ON session_messages BEGIN
    INSERT INTO session_messages_fts(rowid, session_id, role, content) VALUES (new.id, new.session_id, new.role, new.content);
END;

CREATE TRIGGER IF NOT EXISTS session_messages_fts_delete AFTER DELETE ON session_messages BEGIN
    INSERT INTO session_messages_fts(session_messages_fts, rowid, session_id, role, content) VALUES('delete', old.id, old.session_id, old.role, old.content);
END;
```

### 数据流

1. FTS5 MATCH 查询获取匹配的 message rowids
2. GROUP BY session_id，限制 top N sessions
3. 对每个 session，获取其完整/截断 conversation（~100k chars）
4. 调用 LLM summarization（Gemini Flash via existing LLM provider）
5. 返回 `SessionSearchResult` 列表

### 写入接口

```json
{
  "name": "session_remember",
  "description": "Store a message in session history for future search.",
  "parameters": {
    "type": "object",
    "properties": {
      "session_id": { "type": "string" },
      "role": { "type": "string" },
      "content": { "type": "string" }
    },
    "required": ["session_id", "role", "content"]
  }
}
```

### 文件结构

```
crates/hermes-tools-extended/src/
└── memory.rs   # 修改：新增 search_sessions(), session_remember(), session_messages FTS
```

---

## 2. ImageGenerationTool

### 目标

使用 Fal.ai FLUX 2 Pro 生成图像，自动 2x upscaling via Fal.ai Clarity Upscaler。

### 核心类型

```rust
// ImageGenerationTool — 单例
pub struct ImageGenerationTool {
    http_client: reqwest::Client,
    fal_api_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationConfig {
    pub prompt: String,
    pub image_size: ImageSize,   // landscape_16_9, portrait_9_16, square_1_1, landscape_4_3
    pub num_inference_steps: u32,  // default 50
    pub guidance_scale: f32,       // default 4.5
    pub num_images: u32,          // default 1
    pub enable_safety_checker: bool, // default false
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
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
```

### 工具接口

```json
{
  "name": "image_generate",
  "description": "Generate images from text prompts using Fal.ai FLUX 2 Pro with automatic 2x upscaling.",
  "parameters": {
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
  }
}
```

### Fal.ai API 流程

```
1. POST https://queue.fal.run/fal-ai/flux-2-pro
   → 获得 request_id

2. GET/POST polling https://queue.fal.run/fal-ai/flux-2-pro/results?request_id={id}
   → 获得图像 URL

3. POST https://queue.fal.run/fal-ai/clarity-upscaler
   → 对每张图像做 2x upscaling

4. 下载并返回 base64 编码的最终图像
```

### 响应格式

```json
{
  "success": true,
  "images": [
    {
      "url": "data:image/png;base64,...",
      "width": 2048,
      "height": 1536,
      "upscaled": true
    }
  ],
  "model": "fal-ai/flux-2-pro"
}
```

### 文件结构

```
crates/hermes-tools-extended/src/
└── image_generation.rs   # ImageGenerationTool
```

---

## 3. TranscriptionTool

### 目标

语音转文字，支持 faster-whisper 本地（无需 API key）和 Groq Whisper API（免费 tier）。

### 核心类型

```rust
// TranscriptionTool — 单例
pub struct TranscriptionTool {
    http_client: reqwest::Client,
    groq_api_key: Option<String>,
    whisper_model_path: Option<PathBuf>,  // faster-whisper 模型路径
}

pub enum TranscriptionProvider {
    FasterWhisper,  // 本地，无 API key
    GroqWhisper,    // Groq API，免费 tier
}
```

### 工具接口

```json
{
  "name": "transcribe",
  "description": "Transcribe audio to text. Supports faster-whisper (local) and Groq Whisper API.",
  "parameters": {
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
  }
}
```

### faster-whisper 子进程调用

```rust
// 调用 faster-whisper CLI（ctranslate2）
let model_path = self.whisper_model_path.as_deref()
    .unwrap_or_else(|| PathBuf::from("~/.cache/faster-whisper").as_path());

let mut cmd = Command::new("whisper");
cmd.arg("--model").arg(model_path)
   .arg("--language").arg(lang)
   .arg("--output_format").arg("json")
   .arg("--input").arg(audio_path);
```

### Groq API

```rust
let resp = self.http_client
    .post("https://api.groq.com/v1/audio/transcriptions")
    .header("Authorization", format!("Bearer {}", self.groq_api_key))
    .form(&[
        ("file", fs::File::open(audio_path)?),
        ("model", "whisper-large-v3"),
        ("language", lang),
    ])
    .send()
    .await?;
```

### 支持的音频格式

`mp3, mp4, mpeg, mpga, m4a, wav, webm, ogg, aac`

### 文件结构

```
crates/hermes-tools-extended/src/
└── transcription.rs   # TranscriptionTool
```

---

## 4. HomeAssistantTool

### 目标

控制 Home Assistant 智能家居设备，四个工具：list_entities、get_state、list_services、call_service。支持 auto-discovery。

### 核心类型

```rust
// HomeAssistantTool — 单例
pub struct HomeAssistantTool {
    http_client: reqwest::Client,
    hass_url: String,
    hass_token: Option<String>,
    discovery_cache: Arc<RwLock<Option<DiscoveredInstance>>>,
}

struct DiscoveredInstance {
    url: String,
    name: String,
}
```

### 工具接口（4 个 action）

```json
{
  "name": "homeassistant",
  "description": "Control Home Assistant devices. Supports ha_list_entities, ha_get_state, ha_list_services, ha_call_service.",
  "parameters": {
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
  }
}
```

### Auto-Discovery (mDNS/SSDP)

```rust
impl HomeAssistantTool {
    pub async fn discover(&self) -> Result<Option<DiscoveredInstance>, ToolError> {
        // mDNS: 查询 _home-assistant._tcp.local
        // SSDP: 发送 M-SEARCH 到 239.255.255.250:1900
        // 如果找到，更新 hass_url 并返回
    }
}
```

### 安全验证

```rust
// Entity ID 格式验证（严格 regex）
static ENTITY_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z_][a-z0-9_]*\.[a-z0-9_]+$").unwrap()
});

// Service name 验证（无路径遍历）
static SERVICE_NAME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[a-z][a-z0-9_]*$").unwrap()
});

// 禁止的 domains（SSRF/命令执行风险）
static BLOCKED_DOMAINS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    ["shell_command", "command_line", "python_script", "pyscript", "hassio", "rest_command"]
        .into_iter().collect()
});
```

### 文件结构

```
crates/hermes-tools-extended/src/
└── homeassistant.rs   # HomeAssistantTool
```

---

## 5. MixtureOfAgentsTool

### 目标

多 LLM 并行聚合：调用多个 reference models 生成多样化响应，通过 aggregator model 合成最终答案。Via OpenRouter API。

### 核心类型

```rust
// MixtureOfAgentsTool — 单例
pub struct MixtureOfAgentsTool {
    http_client: reqwest::Client,
    openrouter_api_key: String,
}

pub struct MoAConfig {
    pub reference_models: Vec<String>,  // 默认: claude-opus-4-5-sonnet-20241022, gemini-2-5-pro-preview, gpt-5-pro, deepseek-v3
    pub aggregator_model: String,         // 默认: claude-opus-4-5-sonnet-20241022
    pub reference_temperature: f32,       // default 0.7
    pub aggregator_temperature: f32,       // default 0.3
    pub min_successful_references: usize, // default 2
}

impl Default for MoAConfig {
    fn default() -> Self {
        Self {
            reference_models: vec![
                "anthropic/claude-opus-4-5-sonnet-20241022".to_string(),
                "google/gemini-2.5-pro-preview-06-05".to_string(),
                "openai/gpt-5-pro".to_string(),
                "deepseek/deepseek-v3".to_string(),
            ],
            aggregator_model: "anthropic/claude-opus-4-5-sonnet-20241022".to_string(),
            reference_temperature: 0.7,
            aggregator_temperature: 0.3,
            min_successful_references: 2,
        }
    }
}
```

### 工具接口

```json
{
  "name": "mixture_of_agents",
  "description": "Solve complex queries using multiple frontier LLMs in parallel, synthesized by an aggregator.",
  "parameters": {
    "type": "object",
    "properties": {
      "prompt": { "type": "string" },
      "reference_models": { "type": "array", "items": { "type": "string" } },
      "aggregator_model": { "type": "string" }
    },
    "required": ["prompt"]
  }
}
```

### 执行流程

```
1. 并行调用所有 reference models（via OpenRouter）
2. 等待所有 responses（或超时）
3. 丢弃失败的，保留至少 min_successful_references 个
4. 将所有 reference responses 组合为 aggregator prompt
5. 调用 aggregator model
6. 返回最终合成答案
```

### OpenRouter 调用

```rust
async fn call_openrouter(&self, model: &str, prompt: &str, temperature: f32) -> Result<String, ToolError> {
    let resp = self.http_client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", self.openrouter_api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": temperature
        }))
        .send()
        .await?;
    // parse response...
}
```

### 响应格式

```json
{
  "success": true,
  "answer": "The synthesized answer from the aggregator...",
  "reference_count": 4,
  "references": [
    { "model": "claude-opus-4-5-sonnet-20241022", "excerpt": "..." },
    { "model": "gemini-2.5-pro-preview-06-05", "excerpt": "..." }
  ],
  "aggregator": "claude-opus-4-5-sonnet-20241022"
}
```

### 文件结构

```
crates/hermes-tools-extended/src/
└── mixture_of_agents.rs   # MixtureOfAgentsTool
```

---

## 依赖

```toml
# hermes-tools-extended/Cargo.toml
reqwest.workspace = true
tokio.workspace = true
parking_lot.workspace = true
async-trait.workspace = true
hermes-core.workspace = true
hermes-memory.workspace = true
hermes-tool-registry.workspace = true

# 新增
rusqlite.workspace = true   # 如果 memory.rs 需要额外 FTS 表（如果需要独立 session_messages 表）
# 如果 session_messages 作为独立表，需要添加 rusqlite

# 注意：已有 tempfile = "3" 在 Cargo.toml 中（code_execution 使用）
```

---

## 实现顺序与文件位置

| 工具 | Crate | 优先 | 依赖 |
|------|-------|------|------|
| SessionSearchTool | hermes-tools-extended | 1 | MemoryTool existing |
| ImageGenerationTool | hermes-tools-extended | 2 | None |
| TranscriptionTool | hermes-tools-extended | 3 | None |
| HomeAssistantTool | hermes-tools-extended | 4 | None |
| MixtureOfAgentsTool | hermes-tools-extended | 5 | None |

---

## 与 Python 版的主要差异

| 方面 | Python 版 | Rust 版（本文） |
|------|----------|----------------|
| SessionSearch | SQLite FTS + Gemini Flash | SQLite FTS + configurable LLM aggregator |
| ImageGeneration | Fal.ai FLUX 2 Pro | Fal.ai FLUX 2 Pro（相同） |
| Transcription | faster-whisper + Groq + OpenAI | faster-whisper + Groq（无 OpenAI） |
| HomeAssistant | mDNS/SSDP discovery + token auth | mDNS/SSDP discovery + token auth（相同） |
| MixtureOfAgents | OpenRouter + configurable | OpenRouter + configurable（相同） |
| 并发 | asyncio | tokio::spawn + join! |
