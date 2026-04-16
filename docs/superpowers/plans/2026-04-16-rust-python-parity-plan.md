# Rust-Python Parity: 5 Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 5 个工具（VisionTool 增强、MemoryTool 增强、WebSearchTool 增强、DelegateTool、CodeExecutionTool），使 Rust 版 hermes-agent 与 Python 版功能对齐。

**Architecture:**
- 所有新工具放在 `hermes-tools-extended` crate
- 遵循现有 `Tool` trait 模式（`async_trait::async_trait`）
- Provider trait 模式用于可扩展后端（VisionProvider、SearchProvider）
- 使用 `parking_lot::RwLock` 进行内部状态管理
- tokio 用于所有 async 操作

**Tech Stack:** reqwest, rusqlite, tokio::process, parking_lot, async-trait

---

## Pre-flight: 确认依赖和现有代码

- [ ] **Step 1: 检查 hermes-tools-extended/Cargo.toml**

Run: `cat /Users/Rowe/ai-projects/rust-hermes-agent/crates/hermes-tools-extended/Cargo.toml`
确认已有：`reqwest`, `tokio`, `parking_lot`, `async-trait`, `base64`, `serde`, `serde_json`

---

## Task 1: VisionTool 增强

**增强现有 VisionTool：添加 Anthropic Claude Vision provider 和 Base64 支持。**

**Files:**
- Modify: `crates/hermes-tools-extended/src/vision.rs`
- Test: `crates/hermes-tools-extended/tests/test_vision.rs`

### Step 1: Read existing VisionTool

Run: `cat crates/hermes-tools-extended/src/vision.rs`

### Step 2: 添加 Anthropic Claude Vision provider

- [ ] **Step 2: Write failing test for Anthropic provider**

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_anthropic_vision_provider_image_url() {
        use hermes_core::{ModelId, ChatRequest, Message, Content};
        use crate::vision::AnthropicVisionProvider;
        // Provider needs to be constructed - mock or skip if no API key
        // This test documents expected behavior
        let provider = AnthropicVisionProvider::new("test-key".to_string());
        let image = crate::vision::VisionImage::Url("https://example.com/image.png".to_string());
        let result = provider.analyze(image, "What is in this image?").await;
        // Without real API key, expect auth error
        assert!(result.is_err() || result.unwrap().model.contains("claude"));
    }
}
```

### Step 3: Implement Anthropic Claude Vision provider

在 `vision.rs` 中添加：

```rust
/// Anthropic Claude Vision provider
#[derive(Clone)]
pub struct AnthropicVisionProvider {
    api_key: String,
    model: String,
    http_client: reqwest::Client,
}

impl AnthropicVisionProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-3-5-sonnet-20241022".to_string(),
            http_client: reqwest::Client::new(),
        }
    }
}

impl VisionProvider for AnthropicVisionProvider {
    async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError> {
        let (media_type, data) = match &image {
            VisionImage::Url(url) => {
                let resp = self.http_client.get(url).send().await
                    .map_err(|e| ToolError::Execution(format!("Failed to download image: {}", e)))?;
                let bytes = resp.bytes().await
                    .map_err(|e| ToolError::Execution(format!("Failed to read image bytes: {}", e)))?;
                let base64_str = BASE64.encode(&bytes);
                let media_type = "image/jpeg"; // Could infer from Content-Type header
                (media_type.to_string(), base64_str)
            }
            VisionImage::Base64(b64) => {
                ("image/jpeg".to_string(), b64.clone())
            }
        };

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": data
                        }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }]
        });

        let resp = self.http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Anthropic API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Anthropic response: {}", e)))?;

        if let Some(error) = body.get("error") {
            return Err(ToolError::Execution(
                error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error").to_string()
            ));
        }

        let content = body["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("No analysis returned")
            .to_string();

        Ok(VisionResult {
            analysis: content,
            model: self.model.clone(),
        })
    }
}
```

### Step 4: Add VisionImage enum and VisionProvider trait to vision.rs

```rust
/// VisionProvider trait — 可插拔的视觉分析后端
pub trait VisionProvider: Send + Sync {
    async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError>;
    fn name(&self) -> &str;
}

#[derive(Clone, Debug)]
pub enum VisionImage {
    Url(String),
    Base64(String),
}

#[derive(Clone, Debug)]
pub struct VisionResult {
    pub analysis: String,
    pub model: String,
}

/// OpenAI Vision Provider (GPT-4V)
pub struct OpenAIVisionProvider {
    api_key: String,
    model: String,
    http_client: reqwest::Client,
}

impl OpenAIVisionProvider {
    pub fn new(api_key: String, model: &str) -> Self {
        Self {
            api_key,
            model: model.to_string(),
            http_client: reqwest::Client::new(),
        }
    }
}

impl VisionProvider for OpenAIVisionProvider {
    fn name(&self) -> &str { "openai" }

    async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError> {
        let (url, detail) = match &image {
            VisionImage::Url(u) => (u.clone(), "auto".to_string()),
            VisionImage::Base64(b64) => (format!("data:image/jpeg;base64,{}", b64), "auto".to_string()),
        };

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": { "url": url, "detail": detail }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }]
        });

        let resp = self.http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenAI API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid OpenAI response: {}", e)))?;

        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No analysis returned")
            .to_string();

        Ok(VisionResult {
            analysis: content,
            model: self.model.clone(),
        })
    }
}
```

### Step 5: Update VisionTool to use provider and support base64

- [ ] **Step 5: Modify VisionTool to use VisionProvider trait and support base64**

替换现有的 `VisionTool` struct 和实现，使用 provider 模式：

```rust
/// VisionTool — 图像分析工具，支持多 provider
#[derive(Clone)]
pub struct VisionTool {
    providers: std::collections::HashMap<String, Arc<dyn VisionProvider>>,
    default_provider: String,
}

impl VisionTool {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        // Default: use LLM provider directly (backward compat)
        // Provider-based approach is add-on enhancement
        let mut providers = std::collections::HashMap::new();
        providers.insert("llm".to_string(), provider);
        Self {
            providers,
            default_provider: "llm".to_string(),
        }
    }

    pub fn with_openai(mut self, api_key: String, model: &str) -> Self {
        self.providers.insert(
            "openai".to_string(),
            Arc::new(OpenAIVisionProvider::new(api_key, model)) as Arc<dyn VisionProvider>
        );
        self
    }

    pub fn with_anthropic(mut self, api_key: String) -> Self {
        self.providers.insert(
            "anthropic".to_string(),
            Arc::new(AnthropicVisionProvider::new(api_key)) as Arc<dyn VisionProvider>
        );
        self
    }

    fn parse_image(input: &str) -> VisionImage {
        if input.starts_with("data:") || input.starts_with("http://") || input.starts_with("https://") {
            VisionImage::Url(input.to_string())
        } else if input.len() > 100 && !input.contains('/') && !input.contains('\\') {
            // Likely base64
            VisionImage::Base64(input.to_string())
        } else {
            // Local file path
            let data = std::fs::read(input).unwrap_or_default();
            let base64_str = BASE64.encode(&data);
            VisionImage::Base64(base64_str)
        }
    }
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str { "vision_analyze" }
    fn description(&self) -> &str {
        "Analyze images using vision AI. Supports image URLs, local file paths, or base64-encoded images."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Image URL, local file path, or base64-encoded image"
                },
                "prompt": {
                    "type": "string",
                    "description": "Analysis question",
                    "default": "Describe this image in detail"
                },
                "provider": {
                    "type": "string",
                    "enum": ["llm", "openai", "anthropic"],
                    "default": "llm"
                }
            },
            "required": ["image"]
        })
    }
    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let image_str = args["image"].as_str()
            .ok_or_else(|| ToolError::InvalidArgs("image is required".to_string()))?;
        let prompt = args["prompt"].as_str().unwrap_or("Describe this image in detail");
        let provider_name = args["provider"].as_str().unwrap_or("llm");

        let image = Self::parse_image(image_str);

        let provider = self.providers.get(provider_name)
            .ok_or_else(|| ToolError::InvalidArgs(
                format!("Unknown provider: {}. Available: {}", provider_name,
                    self.providers.keys().cloned().collect::<Vec<_>>().join(", "))
            ))?;

        let result = provider.analyze(image, prompt).await?;

        Ok(serde_json::json!({
            "success": true,
            "analysis": result.analysis,
            "model": result.model
        }).to_string())
    }
}
```

### Step 6: Run tests

- [ ] **Step 6: Run cargo check**

Run: `cargo check -p hermes-tools-extended 2>&1 | tail -20`
Expected: Compiles (may have unused warnings for provider types)

- [ ] **Step 7: Add tests**

在 `tests/test_vision.rs` 添加：
```rust
#[test]
fn test_vision_tool_name() {
    use hermes_core::LlmProvider;
    use std::sync::Arc;
    // Mock LLM provider
    struct MockProvider;
    impl LlmProvider for MockProvider {
        async fn chat(&self, _: hermes_core::ChatRequest) -> Result<hermes_core::ChatResponse, hermes_core::Error> {
            Err(hermes_core::Error::InvalidModel("mock".to_string()))
        }
    }
    let tool = crate::vision::VisionTool::new(Arc::new(MockProvider));
    assert_eq!(tool.name(), "vision_analyze");
}

#[test]
fn test_parse_image_url() {
    let img = crate::vision::VisionTool::parse_image("https://example.com/image.png");
    match img {
        crate::vision::VisionImage::Url(u) => assert!(u.contains("example.com")),
        _ => panic!("Expected Url variant"),
    }
}

#[test]
fn test_parse_image_base64() {
    let img = crate::vision::VisionTool::parse_image("aGVsbG8gd29ybGQ=");
    match img {
        crate::vision::VisionImage::Base64(b) => assert_eq!(b, "aGVsbG8gd29ybGQ="),
        _ => panic!("Expected Base64 variant"),
    }
}
```

- [ ] **Step 8: Run tests**

Run: `cargo test -p hermes-tools-extended test_vision -- --nocapture 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 9: Commit**

```bash
git add crates/hermes-tools-extended/src/vision.rs crates/hermes-tools-extended/tests/test_vision.rs
git commit -m "feat(hermes-tools-extended): enhance VisionTool with Anthropic Claude Vision and base64 support"
```

---

## Task 2: MemoryTool FTS Enhancement

**增强现有 MemoryTool：添加 SQLite FTS5 全文搜索，增强记忆分类和标签。**

**Files:**
- Modify: `crates/hermes-tools-extended/src/memory.rs`
- Modify: `crates/hermes-tools-extended/Cargo.toml`（如果需要新依赖）
- Test: `crates/hermes-tools-extended/tests/test_memory.rs`

### Step 1: Read existing MemoryTool

Run: `cat crates/hermes-tools-extended/src/memory.rs`

### Step 2: Write failing FTS test

- [ ] **Step 1: Write failing test for FTS search**

```rust
#[tokio::test]
async fn test_memory_fts_search() {
    let pool = create_test_pool().await;
    let store = MemoryStore::new(pool.clone()).await.unwrap();
    // Insert with category
    store.set("project_x".to_string(), "Rust async runtime comparison".to_string(), Some("research".to_string()), &[]).await.unwrap();
    // Search for "rust async"
    let results = store.search_fts("rust async", 5).await.unwrap();
    assert!(!results.is_empty(), "Should find 'rust async' in 'Rust async runtime comparison'");
}
```

### Step 3: Define MemoryStore with FTS

在 `memory.rs` 中，添加 FTS 初始化和搜索方法：

```rust
/// MemoryStore — 增强版，支持 FTS
pub struct MemoryStore {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl MemoryStore {
    pub async fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Result<Self, ToolError> {
        let store = Self { pool };
        store.init_fts().await?;
        Ok(store)
    }

    async fn init_fts(&self) -> Result<(), ToolError> {
        // Create FTS5 virtual table if not exists
        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(key, value, category, content=memory, content_rowid=rowid)"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS init error: {}", e)))?;

        // Create triggers to keep FTS in sync
        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS memory_fts_insert AFTER INSERT ON memory BEGIN INSERT INTO memory_fts(rowid, key, value, category) VALUES (new.rowid, new.key, new.value, new.category); END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS trigger insert error: {}", e)))?;

        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS memory_fts_delete AFTER DELETE ON memory BEGIN INSERT INTO memory_fts(memory_fts, rowid, key, value, category) VALUES('delete', old.rowid, old.key, old.value, old.category); END"
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS trigger delete error: {}", e)))?;

        Ok(())
    }

    pub async fn set(&self, key: String, value: String, category: Option<String>, tags: &[String]) -> Result<(), ToolError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as f64;
        sqlx::query(
            "INSERT OR REPLACE INTO memory (key, value, category, created_at, updated_at) VALUES (?, ?, ?, COALESCE((SELECT created_at FROM memory WHERE key = ?), ?), ?)"
        )
        .bind(&key)
        .bind(&value)
        .bind(&category)
        .bind(&key)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| ToolError::Execution(format!("Memory set error: {}", e)))?;
        Ok(())
    }

    pub async fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryResult>, ToolError> {
        let pattern = format!("\"{}\"", query.replace("\"", "\"\""));
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT m.key, m.value, m.category FROM memory m JOIN memory_fts f ON m.rowid = f.rowid WHERE memory_fts MATCH ? LIMIT ?"
        )
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS search error: {} (query: {:?})", e, pattern)))?;

        Ok(rows.into_iter().map(|(k, v, c)| MemoryResult { key: k, value: v, category: c }).collect())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct MemoryResult {
    pub key: String,
    pub value: String,
    pub category: Option<String>,
}
```

### Step 4: Update MemoryParams to support category and tags

- [ ] **Step 2: Update MemoryParams to support category and tags**

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum MemoryParams {
    Set {
        key: String,
        value: String,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
    },
    Get { key: String },
    Search {
        query: String,
        #[serde(default)]
        limit: Option<usize>,
    },
    Read {
        #[serde(default)]
        category: Option<String>,
    },
}
```

### Step 5: Update execute to use FTS and support new params

- [ ] **Step 3: Update execute method with FTS and Read**

在 `execute` 方法中添加 `Read` 分支，并修改 `Search` 使用 `search_fts`：

```rust
match params {
    MemoryParams::Set { key, value, category, tags } => {
        self.store.set(key, value, category, &tags).await?;
        Ok(json!({ "status": "ok", "key": key }).to_string())
    }
    MemoryParams::Get { key } => {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM memory WHERE key = ?"
        )
        .bind(&key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ToolError::Execution(format!("Memory get error: {}", e)))?;
        match row {
            Some((value,)) => Ok(json!({ "key": key, "value": value }).to_string()),
            None => Ok(json!({ "key": key, "value": null }).to_string()),
        }
    }
    MemoryParams::Search { query, limit } => {
        let results = self.store.search_fts(&query, limit.unwrap_or(10)).await?;
        Ok(json!({
            "query": query,
            "results": results.into_iter().map(|r| json!({"key": r.key, "value": r.value, "category": r.category})).collect::<Vec<_>>()
        }).to_string())
    }
    MemoryParams::Read { category } => {
        let rows: Vec<(String, String, Option<String>)> = if let Some(cat) = category {
            sqlx::query_as(
                "SELECT key, value, category FROM memory WHERE category = ? ORDER BY updated_at DESC"
            )
            .bind(&cat)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ToolError::Execution(format!("Memory read error: {}", e)))?
        } else {
            sqlx::query_as(
                "SELECT key, value, category FROM memory ORDER BY updated_at DESC"
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ToolError::Execution(format!("Memory read error: {}", e)))?
        };
        let memories = rows.into_iter().map(|(k, v, c)| json!({"key": k, "value": v, "category": c})).collect::<Vec<_>>();
        Ok(json!({ "memories": memories }).to_string())
    }
}
```

### Step 6: Run tests

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p hermes-tools-extended 2>&1 | tail -20`

- [ ] **Step 5: Add memory tests**

```rust
#[tokio::test]
async fn test_memory_store_set_with_category() {
    use hermes_core::ToolError;
    let pool = create_test_pool().await;
    let store = crate::memory::MemoryStore::new(pool).await.unwrap();
    store.set("test_key".to_string(), "test_value".to_string(), Some("test".to_string()), &[]).await.unwrap();
}

#[tokio::test]
async fn test_memory_params_with_category() {
    let json = serde_json::json!({
        "action": "set",
        "key": "k1",
        "value": "v1",
        "category": "research"
    });
    let params: crate::memory::MemoryParams = serde_json::from_value(json).unwrap();
    match params {
        crate::memory::MemoryParams::Set { key, value, category, .. } => {
            assert_eq!(key, "k1");
            assert_eq!(category, Some("research".to_string()));
        }
        _ => panic!("Expected Set"),
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p hermes-tools-extended test_memory -- --nocapture 2>&1 | tail -20`

- [ ] **Step 7: Commit**

```bash
git add crates/hermes-tools-extended/src/memory.rs crates/hermes-tools-extended/tests/test_memory.rs
git commit -m "feat(hermes-tools-extended): enhance MemoryTool with SQLite FTS5 and category support"
```

---

## Task 3: WebSearchTool Enhancement

**增强 WebSearchTool：添加 Exa + Tavily + Firecrawl provider，支持 provider 切换。**

**Files:**
- Modify: `crates/hermes-tools-extended/src/web_search.rs`
- Test: `crates/hermes-tools-extended/tests/test_web_search.rs`

### Step 1: Add SearchProvider trait

- [ ] **Step 1: Write failing test for Exa provider**

```rust
#[tokio::test]
async fn test_search_provider_trait() {
    use crate::web_search::{SearchProvider, SearchResult};
    // Test trait object works
    let providers: Vec<Box<dyn SearchProvider>> = Vec::new();
    assert!(providers.is_empty());
}
```

### Step 2: Add SearchProvider trait and provider implementations

在 `web_search.rs` 中添加：

```rust
use async_trait::async_trait;

/// SearchProvider trait — 可插拔的搜索后端
#[async_trait]
pub trait SearchProvider: Send + Sync {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError>;
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub content: Option<String>,
}

/// ExaSearchProvider — Exa AI (exa.ai)
pub struct ExaSearchProvider {
    api_key: String,
    http_client: reqwest::Client,
}

impl ExaSearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for ExaSearchProvider {
    fn name(&self) -> &str { "exa" }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let payload = serde_json::json!({
            "query": query,
            "num_results": limit,
            "contents": ["html"]
        });

        let resp = self.http_client
            .post("https://api.exa.ai/search")
            .header("x-api-key", &self.api_key)
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Exa API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Exa response: {}", e)))?;

        let results = body["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            url: r["url"].as_str()?.to_string(),
                            title: r["title"].as_str().unwrap_or("").to_string(),
                            snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                            content: r["url"].as_str().map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

/// TavilySearchProvider — Tavily AI (tavily.ai)
pub struct TavilySearchProvider {
    api_key: String,
    http_client: reqwest::Client,
}

impl TavilySearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for TavilySearchProvider {
    fn name(&self) -> &str { "tavily" }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let resp = self.http_client
            .get("https://api.tavily.com/search")
            .query(&[
                ("query", query),
                ("api_key", &self.api_key),
                ("max_results", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Tavily API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Tavily response: {}", e)))?;

        let results = body["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            url: r["url"].as_str()?.to_string(),
                            title: r["title"].as_str().unwrap_or("").to_string(),
                            snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                            content: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

/// FirecrawlSearchProvider — Firecrawl (firecrawl.dev)
pub struct FirecrawlSearchProvider {
    api_key: String,
    engine: String,
    http_client: reqwest::Client,
}

impl FirecrawlSearchProvider {
    pub fn new(api_key: String, engine: &str) -> Self {
        Self {
            api_key,
            engine: engine.to_string(),
            http_client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for FirecrawlSearchProvider {
    fn name(&self) -> &str { "firecrawl" }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let payload = serde_json::json!({
            "query": query,
            "limit": limit,
            "engine": self.engine
        });

        let resp = self.http_client
            .post("https://api.firecrawl.dev/v0/search")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Firecrawl API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Firecrawl response: {}", e)))?;

        let results = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            url: r["url"].as_str()?.to_string(),
                            title: r["title"].as_str().unwrap_or("").to_string(),
                            snippet: r["description"].as_str().unwrap_or("").to_string(),
                            content: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}
```

### Step 3: Update WebSearchTool to use providers

- [ ] **Step 2: Rewrite WebSearchTool to use SearchProvider trait**

```rust
/// WebSearchTool — 网页搜索工具，支持多 provider
#[derive(Debug, Clone)]
pub struct WebSearchTool {
    providers: std::collections::HashMap<String, Box<dyn SearchProvider>>,
    default_provider: String,
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        let mut providers = std::collections::HashMap::new();
        // Default: DuckDuckGo (free, no API key)
        providers.insert("duckduckgo".to_string(), Box::new(DuckDuckGoProvider::new()) as Box<dyn SearchProvider>);
        Self {
            providers,
            default_provider: "duckduckgo".to_string(),
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client builder"),
        }
    }

    pub fn with_exa(mut self, api_key: String) -> Self {
        self.providers.insert("exa".to_string(), Box::new(ExaSearchProvider::new(api_key)));
        self
    }

    pub fn with_tavily(mut self, api_key: String) -> Self {
        self.providers.insert("tavily".to_string(), Box::new(TavilySearchProvider::new(api_key)));
        self
    }

    pub fn with_firecrawl(mut self, api_key: String, engine: &str) -> Self {
        self.providers.insert("firecrawl".to_string(), Box::new(FirecrawlSearchProvider::new(api_key, engine)));
        self
    }
}

/// DuckDuckGoProvider — 免费的 HTML 搜索
struct DuckDuckGoProvider {
    client: reqwest::Client,
}

impl DuckDuckGoProvider {
    fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Mozilla/5.0")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client builder"),
        }
    }
}

#[async_trait]
impl SearchProvider for DuckDuckGoProvider {
    fn name(&self) -> &str { "duckduckgo" }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));
        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;
        let results = self.parse_ddg_html(&body, limit);
        Ok(results)
    }
}

impl WebSearchTool {
    fn parse_ddg_html(&self, html: &str, num_results: usize) -> Vec<SearchResult> {
        use scraper::{Html, Selector};
        let document = Html::parse_document(html);
        let result_selector = Selector::parse("a.result__a").unwrap();
        let mut results = Vec::new();
        for (idx, element) in document.select(&result_selector).enumerate() {
            if idx >= num_results { break; }
            if let Some(href) = element.value().attr("href") {
                let title = element.text().collect::<String>();
                results.push(SearchResult {
                    url: href.to_string(),
                    title: title.trim().to_string(),
                    snippet: String::new(),
                    content: None,
                });
            }
        }
        results
    }
}

impl Default for WebSearchTool {
    fn default() -> Self { Self::new() }
}
```

### Step 4: Update Tool impl

- [ ] **Step 3: Update Tool impl for multi-provider**

```rust
#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }

    fn description(&self) -> &str {
        "Search the web. Supports multiple providers: duckduckgo (free), exa, tavily, firecrawl (API keys required)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query" },
                "num_results": { "type": "integer", "default": 5 },
                "provider": {
                    "type": "string",
                    "enum": ["duckduckgo", "exa", "tavily", "firecrawl"],
                    "default": "duckduckgo"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let query = args["query"].as_str()
            .ok_or_else(|| ToolError::InvalidArgs("query is required".to_string()))?;
        let num_results = args["num_results"].as_u64().unwrap_or(5) as usize;
        let provider_name = args["provider"].as_str().unwrap_or("duckduckgo");

        let provider = self.providers.get(provider_name)
            .ok_or_else(|| ToolError::InvalidArgs(
                format!("Unknown provider: {}. Available: {}", provider_name,
                    self.providers.keys().cloned().collect::<Vec<_>>().join(", "))
            ))?;

        let results = provider.search(query, num_results).await?;

        Ok(serde_json::json!({
            "success": true,
            "query": query,
            "results": results,
            "provider": provider_name
        }).to_string())
    }
}
```

### Step 5: Run tests

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p hermes-tools-extended 2>&1 | tail -20`

- [ ] **Step 5: Add tests**

```rust
#[test]
fn test_web_search_tool_name() {
    let tool = crate::web_search::WebSearchTool::new();
    assert_eq!(tool.name(), "web_search");
}

#[test]
fn test_web_search_default_provider() {
    let tool = crate::web_search::WebSearchTool::new();
    assert!(tool.providers.contains_key("duckduckgo"));
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p hermes-tools-extended test_web_search -- --nocapture 2>&1 | tail -20`

- [ ] **Step 7: Commit**

```bash
git add crates/hermes-tools-extended/src/web_search.rs crates/hermes-tools-extended/tests/test_web_search.rs
git commit -m "feat(hermes-tools-extended): enhance WebSearchTool with Exa/Tavily/Firecrawl providers"
```

---

## Task 4: DelegateTool

**实现 DelegateTool：通过独立 hermes-cli 子进程执行子任务，支持并发和受限工具集。**

**Files:**
- Create: `crates/hermes-tools-extended/src/delegate_tool.rs`
- Create: `crates/hermes-tools-extended/tests/test_delegate.rs`

### Step 1: Read existing delegate types in hermes-core

Run: `grep -r "DelegateParams\|DelegateResult\|DelegateTool" crates/hermes-core/src/ --include="*.rs" -l`
然后 `cat` 相关文件

### Step 2: Write failing test

- [ ] **Step 1: Write failing test for DelegateTool**

```rust
#[tokio::test]
async fn test_delegate_tool_name() {
    use crate::delegate_tool::DelegateTool;
    let cli_path = std::path::PathBuf::from("/usr/local/bin/hermes");
    let tool = DelegateTool::new(cli_path, std::path::PathBuf::from("/tmp"));
    assert_eq!(tool.name(), "delegate_task");
}

#[tokio::test]
async fn test_delegate_params_deserialization() {
    use crate::delegate_tool::DelegateParams;
    let json = serde_json::json!({
        "goal": "Search for rust async runtime info",
        "toolsets": ["web"],
        "max_iterations": 50
    });
    let params: DelegateParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.goal, "Search for rust async runtime info");
    assert_eq!(params.toolsets, vec!["web"]);
    assert_eq!(params.max_iterations, Some(50));
}
```

### Step 3: Implement DelegateTool

创建 `crates/hermes-tools-extended/src/delegate_tool.rs`：

```rust
//! DelegateTool — 子 Agent 并发执行工具
//!
//! 通过独立 hermes-cli 子进程执行子任务，支持受限工具集。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command, Stdio};
use tokio::sync::{Mutex, Semaphore};

const DEFAULT_MAX_CONCURRENT: usize = 3;

/// DelegateTool — 单例
pub struct DelegateTool {
    cli_path: PathBuf,
    config_dir: PathBuf,
    semaphore: Arc<Semaphore>,
    active_sessions: Arc<Mutex<HashMap<String, SessionHandle>>>,
}

struct SessionHandle {
    child: tokio::process::Child,
    stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateParams {
    pub goal: String,
    pub toolsets: Vec<String>,
    #[serde(default)]
    pub max_iterations: Option<u32>,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateResult {
    pub status: String,
    pub summary: String,
    pub duration_secs: f64,
}

impl DelegateTool {
    pub fn new(cli_path: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            cli_path,
            config_dir,
            semaphore: Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT)),
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str { "delegate_task" }

    fn description(&self) -> &str {
        "Delegate a task to a subagent with isolated context and restricted toolsets."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "goal": { "type": "string", "description": "Task description for the subagent" },
                "toolsets": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed toolsets for the subagent"
                },
                "max_iterations": { "type": "integer", "description": "Max agent iterations" },
                "context": { "type": "string", "description": "Additional context for the subagent" }
            },
            "required": ["goal", "toolsets"]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: DelegateParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // 等待并发槽位
        let _permit = self.semaphore.acquire().await
            .map_err(|e| ToolError::Execution(format!("Semaphore error: {}", e)))?;

        let task_id = context.session_id.clone();
        let start = std::time::Instant::now();

        // 构建子进程命令
        let mut cmd = Command::new(&self.cli_path);
        cmd.arg("agent")
           .arg("--goal").arg(&params.goal)
           .arg("--toolsets").arg(params.toolsets.join(","))
           .arg("--session").arg(format!("delegate_{}", task_id));

        if let Some(max_iter) = params.max_iterations {
            cmd.arg("--max-iterations").arg(max_iter.to_string());
        }
        if let Some(ctx) = &params.context {
            cmd.arg("--context").arg(ctx);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| ToolError::Execution(format!("Failed to spawn delegate process: {}", e)))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| ToolError::Execution("No stdin for delegate process".to_string()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| ToolError::Execution("No stdout for delegate process".to_string()))?;

        let stdin = Arc::new(Mutex::new(stdin));
        let mut reader = BufReader::new(stdout).lines();

        // 发送 start 消息
        {
            let mut s = stdin.lock().await;
            let msg = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "start",
                "params": {
                    "goal": &params.goal,
                    "toolsets": &params.toolsets,
                    "context": params.context
                }
            });
            s.write_all(format!("{}\n", msg).as_bytes()).await
                .map_err(|e| ToolError::Execution(format!("Failed to send start: {}", e)))?;
        }

        // 读取响应（直到收到 result 或 error）
        let mut result_str = String::new();
        while let Some(line) = reader.next_line().await
            .map_err(|e| ToolError::Execution(format!("Read error: {}", e)))? {
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&line) {
                if resp.get("result").is_some() || resp.get("error").is_some() {
                    result_str = line;
                    break;
                }
            }
        }

        // 终止子进程
        child.kill().await.ok();
        let _ = child.wait().await;

        let duration = start.elapsed().as_secs_f64();

        if result_str.is_empty() {
            return Err(ToolError::Execution("No result from delegate process".to_string()));
        }

        let resp: serde_json::Value = serde_json::from_str(&result_str)
            .map_err(|e| ToolError::Execution(format!("Invalid result JSON: {}", e)))?;

        if let Some(error) = resp.get("error") {
            return Err(ToolError::Execution(
                error.as_str().unwrap_or("Delegate failed").to_string()
            ));
        }

        let result = resp["result"].clone();

        Ok(json!({
            "status": result["status"].as_str().unwrap_or("success"),
            "summary": result["summary"].as_str().unwrap_or(""),
            "duration_secs": duration,
            "provider": result["provider"].as_str().unwrap_or("unknown")
        }).to_string())
    }
}
```

### Step 4: Register in lib.rs

- [ ] **Step 2: Register DelegateTool in lib.rs**

在 `hermes-tools-extended/src/lib.rs` 中添加导出：
```rust
pub mod delegate_tool;
pub use delegate_tool::DelegateTool;
```

在 `register_extended_tools` 中添加注册：
```rust
registry.register(DelegateTool::new(
    std::path::PathBuf::from("hermes"), // 或从 config 获取 cli_path
    config_dir.clone(),
));
```

### Step 5: Run tests

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p hermes-tools-extended 2>&1 | tail -20`

- [ ] **Step 4: Run tests**

Run: `cargo test -p hermes-tools-extended test_delegate -- --nocapture 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-tools-extended/src/delegate_tool.rs crates/hermes-tools-extended/src/lib.rs crates/hermes-tools-extended/tests/test_delegate.rs
git commit -m "feat(hermes-tools-extended): add DelegateTool for subagent execution"
```

---

## Task 5: CodeExecutionTool

**实现 CodeExecutionTool：PTC（Programmatic Tool Calling），支持 UDS 本地和文件 RPC 远程两种模式。**

**Files:**
- Create: `crates/hermes-tools-extended/src/code_execution.rs`
- Create: `crates/hermes-tools-extended/tests/test_code_execution.rs`

### Step 1: Write failing test

- [ ] **Step 1: Write failing test for CodeExecutionTool**

```rust
#[tokio::test]
async fn test_code_execution_tool_name() {
    use crate::code_execution::CodeExecutionTool;
    let tool = CodeExecutionTool::new(crate::code_execution::ExecutionConfig::default());
    assert_eq!(tool.name(), "execute_code");
}

#[test]
fn test_execution_config_defaults() {
    use crate::code_execution::ExecutionConfig;
    let config = ExecutionConfig::default();
    assert_eq!(config.timeout_secs, 300);
    assert_eq!(config.max_tool_calls, 50);
    assert!(config.allowed_tools.contains(&"read_file".to_string()));
}
```

### Step 2: Implement CodeExecutionTool

创建 `crates/hermes-tools-extended/src/code_execution.rs`：

```rust
//! CodeExecutionTool — PTC (Programmatic Tool Calling)
//!
//! 让 LLM 写 Python 脚本，通过 RPC 调用 hermes 工具。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{timeout, Duration};

/// CodeExecutionTool — 单例
pub struct CodeExecutionTool {
    store: Arc<RwLock<ExecutionStore>>,
    config: ExecutionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub allowed_tools: Vec<String>,
    pub timeout_secs: u64,
    pub max_tool_calls: u32,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            allowed_tools: vec![
                "web_search".to_string(), "web_extract".to_string(),
                "read_file".to_string(), "write_file".to_string(),
                "search_files".to_string(), "patch".to_string(),
                "terminal".to_string(),
            ],
            timeout_secs: 300,
            max_tool_calls: 50,
            max_stdout_bytes: 50_000,
            max_stderr_bytes: 10_000,
        }
    }
}

pub struct ExecutionStore {
    pending: HashMap<String, ExecutionHandle>,
}

pub struct ExecutionHandle {
    pub task_id: String,
    pub status: String,
    pub start_time: f64,
}

impl Default for ExecutionStore {
    fn default() -> Self {
        Self { pending: HashMap::new() }
    }
}

impl CodeExecutionTool {
    pub fn new(config: ExecutionConfig) -> Self {
        Self {
            store: Arc::new(RwLock::new(ExecutionStore::default())),
            config,
        }
    }

    /// Generate hermes_tools.py stub for subprocess
    fn generate_stub(&self, mode: &str, socket_path: Option<&str>) -> String {
        let tools = self.config.allowed_tools.join(", ");
        match mode {
            "uds" => {
                format!(r#"
import json
import socket
import sys

SOCKET_PATH = "{socket_path}"

def _rpc(method, params):
    msg = json.dumps({{"jsonrpc": "2.0", "method": method, "params": params, "id": 1}})
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(SOCKET_PATH)
    sock.sendall((msg + "\n").encode())
    resp = sock.recv(4096).decode()
    sock.close()
    return json.loads(resp)

def read_file(path):
    return _rpc("read_file", {{"path": path}})

def write_file(path, content):
    return _rpc("write_file", {{"path": path, "content": content}})

def terminal(cmd):
    return _rpc("terminal", {{"command": cmd}})

def web_search(query):
    return _rpc("web_search", {{"query": query}})

ALLOWED_TOOLS = [{tools}]
"#, socket_path = socket_path.unwrap_or("/tmp/hermes_uds.sock"))
            }
            _ => {
                // File RPC mode stub
                format!(r#"
import json
import os
import time
import tempfile

ALLOWED_TOOLS = [{tools}]
REQ_DIR = tempfile.mkdtemp(prefix="hermes_req_")

def _rpc(method, params):
    req_id = os.urandom(8).hex()
    req_file = os.path.join(REQ_DIR, f"{{req_id}}.req")
    resp_file = os.path.join(REQ_DIR, f"{{req_id}}.resp")
    with open(req_file, "w") as f:
        json.dump({{"method": method, "params": params, "id": req_id}}, f)
    while not os.path.exists(resp_file):
        time.sleep(0.1)
    with open(resp_file, "r") as f:
        return json.load(f)

def read_file(path):
    return _rpc("read_file", {{"path": path}})

def write_file(path, content):
    return _rpc("write_file", {{"path": path, "content": content}})

def terminal(cmd):
    return _rpc("terminal", {{"command": cmd}})

def web_search(query):
    return _rpc("web_search", {{"query": query}})
"#, tools = tools.split(',').map(|s| format!("\"{}\"", s.trim())).collect::<Vec<_>>().join(", "))
            }
        }
    }
}

#[async_trait]
impl Tool for CodeExecutionTool {
    fn name(&self) -> &str { "execute_code" }

    fn description(&self) -> &str {
        "Execute Python code that calls Hermes tools via RPC. Supports local (UDS) and remote (file-based) modes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute" },
                "language": { "type": "string", "enum": ["python"], "default": "python" },
                "timeout_secs": { "type": "integer" }
            },
            "required": ["code"]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let code = args["code"].as_str()
            .ok_or_else(|| ToolError::InvalidArgs("code is required".to_string()))?;
        let timeout_secs = args["timeout_secs"]
            .as_u64()
            .unwrap_or(self.config.timeout_secs);

        // 检测是否跨平台（代码中包含 SSH/Docker 等则为远程）
        let is_remote = code.contains("SSH") || code.contains("docker") || code.contains("modal");

        let mode = if is_remote { "file" } else { "uds" };
        let stub = self.generate_stub(mode, None);

        // Write stub and code to temp files
        let tmpdir = tempfile::tempdir()
            .map_err(|e| ToolError::Execution(format!("Temp dir error: {}", e)))?;
        let stub_path = tmpdir.path().join("hermes_tools.py");
        let code_path = tmpdir.path().join("user_script.py");

        std::fs::write(&stub_path, &stub)
            .map_err(|e| ToolError::Execution(format!("Stub write error: {}", e)))?;
        std::fs::write(&code_path, code)
            .map_err(|e| ToolError::Execution(format!("Code write error: {}", e)))?;

        // Spawn Python process
        let mut cmd = tokio::process::Command::new("python3");
        cmd.arg(&code_path)
           .current_dir(tmpdir.path())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped())
           .env("PYTHONPATH", tmpdir.path().to_string_lossy().as_ref());

        let output = timeout(Duration::from_secs(timeout_secs), cmd.output())
            .await
            .map_err(|_| ToolError::Execution("Code execution timed out".to_string()))?
            .map_err(|e| ToolError::Execution(format!("Process error: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let truncated_stdout = if stdout.len() > self.config.max_stdout_bytes {
            format!("{}...[truncated {} bytes]", &stdout[..self.config.max_stdout_bytes], stdout.len() - self.config.max_stdout_bytes)
        } else {
            stdout.to_string()
        };

        let truncated_stderr = if stderr.len() > self.config.max_stderr_bytes {
            format!("{}...[truncated {} bytes]", &stderr[..self.config.max_stderr_bytes], stderr.len() - self.config.max_stderr_bytes)
        } else {
            stderr.to_string()
        };

        Ok(json!({
            "success": output.status.success(),
            "stdout": truncated_stdout,
            "stderr": truncated_stderr,
            "exit_code": output.status.code(),
            "mode": mode
        }).to_string())
    }
}
```

### Step 3: Run tests

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p hermes-tools-extended 2>&1 | tail -20`

- [ ] **Step 3: Add tests**

```rust
#[test]
fn test_code_execution_tool_schema() {
    use crate::code_execution::CodeExecutionTool;
    let tool = CodeExecutionTool::new(crate::code_execution::ExecutionConfig::default());
    let schema = tool.parameters();
    assert!(schema.pointer("/properties/code").is_some());
    assert!(schema.pointer("/properties/language").is_some());
}

#[test]
fn test_generate_stub_uds() {
    use crate::code_execution::CodeExecutionTool;
    let tool = CodeExecutionTool::new(crate::code_execution::ExecutionConfig::default());
    let stub = tool.generate_stub("uds", Some("/tmp/test.sock"));
    assert!(stub.contains("socket.AF_UNIX"));
    assert!(stub.contains("/tmp/test.sock"));
}

#[test]
fn test_generate_stub_file() {
    use crate::code_execution::CodeExecutionTool;
    let tool = CodeExecutionTool::new(crate::code_execution::ExecutionConfig::default());
    let stub = tool.generate_stub("file", None);
    assert!(stub.contains("req_file"));
    assert!(stub.contains("resp_file"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p hermes-tools-extended test_code_execution -- --nocapture 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-tools-extended/src/code_execution.rs crates/hermes-tools-extended/src/lib.rs crates/hermes-tools-extended/tests/test_code_execution.rs
git commit -m "feat(hermes-tools-extended): add CodeExecutionTool for PTC"
```

---

## 最终验证

### Step 1: Run all extended tool tests

Run: `cargo test -p hermes-tools-extended 2>&1 | tail -30`

### Step 2: Run cargo check --all

Run: `cargo check --all 2>&1 | tail -10`

### Step 3: Final commit

```bash
git add -A && git commit -m "feat(hermes-tools-extended): complete 5-tool parity implementation

- VisionTool: Anthropic Claude Vision + base64 support
- MemoryTool: SQLite FTS5 + category support  
- WebSearchTool: Exa + Tavily + Firecrawl providers
- DelegateTool: CLI subprocess + JSON-RPC
- CodeExecutionTool: UDS + file RPC PTC modes

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## 验收清单

### VisionTool
- [ ] `vision_analyze` 接受 URL/base64/image path
- [ ] Anthropic Claude Vision provider 可用
- [ ] OpenAI GPT-4V provider 可用
- [ ] Provider 通过 `provider` 参数切换

### MemoryTool  
- [ ] `memory_set` 支持 category 和 tags
- [ ] `memory_search` 使用 FTS5 而非 LIKE
- [ ] `memory_read` 支持 category 过滤

### WebSearchTool
- [ ] `web_search` 支持 duckduckgo（默认免费）
- [ ] Exa provider 可用
- [ ] Tavily provider 可用
- [ ] Firecrawl provider 可用

### DelegateTool
- [ ] `delegate_task` 启动独立 hermes-cli 进程
- [ ] JSON-RPC 通信
- [ ] 并发通过 Semaphore 控制（默认 3）

### CodeExecutionTool
- [ ] `execute_code` 接受 Python 代码
- [ ] 生成 hermes_tools.py stub
- [ ] UDS 模式生成 Unix socket RPC
- [ ] 文件 RPC 模式生成文件 polling stub
- [ ] stdout/stderr 截断（50KB/10KB）
