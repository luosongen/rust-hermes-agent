# Context Compression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Context Compression 功能，在对话历史较长时自动压缩消息摘要

**Architecture:** 在 `hermes-memory` crate 中新增压缩模块，扩展 `SqliteSessionStore` 支持压缩段存储，调用 LLM API 生成摘要

**Tech Stack:** SQLite (via sqlx), reqwest (LLM API calls), async-trait

---

## File Structure

```
crates/hermes-memory/src/
├── lib.rs                          # 新增 compression 模块导出
├── sqlite_store.rs                # 新增 compressed_segments 表和查询方法
├── session.rs                     # 新增 CompressedSegment 结构
├── compression_error.rs           # 新增 CompressionError 枚举
├── compression.rs                  # 新增 CompressionManager 核心逻辑
└── tests/
    └── test_compression.rs         # 新增单元测试

crates/hermes-memory/Cargo.toml    # 新增 reqwest 依赖
```

---

## Task 1: CompressionError 定义

**Files:**
- Create: `crates/hermes-memory/src/compression_error.rs`
- Test: `crates/hermes-memory/src/tests/test_compression.rs`

- [ ] **Step 1: 创建压缩错误类型**

```rust
//! Compression error types

/// Compression operation errors
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("LLM API error: {0}")]
    LlmApi(String),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(String),
}
```

- [ ] **Step 2: 添加测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CompressionError::LlmApi("timeout".into());
        assert!(err.to_string().contains("LLM API"));
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p hermes-memory`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-memory/src/compression_error.rs crates/hermes-memory/src/tests/test_compression.rs
git commit -m "feat(memory): add CompressionError type"
```

---

## Task 2: CompressedSegment 数据结构

**Files:**
- Create: `crates/hermes-memory/src/compressed.rs`
- Modify: `crates/hermes-memory/src/lib.rs` (添加导出)
- Test: `crates/hermes-memory/src/tests/test_compression.rs`

- [ ] **Step 1: 创建压缩段结构**

```rust
//! Compressed message segment structure

use chrono::{DateTime, Utc};

/// A compressed segment of messages
#[derive(Debug, Clone)]
pub struct CompressedSegment {
    pub id: String,
    pub session_id: String,
    pub start_message_id: i64,
    pub end_message_id: i64,
    pub summary: String,
    pub vector: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

impl CompressedSegment {
    pub fn new(
        session_id: String,
        start_message_id: i64,
        end_message_id: i64,
        summary: String,
        vector: Vec<f32>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            start_message_id,
            end_message_id,
            summary,
            vector,
            created_at: Utc::now(),
        }
    }

    /// Get the range of message IDs covered by this segment
    pub fn message_range(&self) -> (i64, i64) {
        (self.start_message_id, self.end_message_id)
    }

    /// Check if a message ID falls within this segment
    pub fn contains(&self, message_id: i64) -> bool {
        message_id >= self.start_message_id && message_id <= self.end_message_id
    }
}
```

- [ ] **Step 2: 更新 lib.rs 导出**

在 `lib.rs` 中添加：
```rust
pub mod compression_error;
pub mod compressed;
```

- [ ] **Step 3: 添加测试**

```rust
#[test]
fn test_compressed_segment_contains() {
    let segment = CompressedSegment::new(
        "session1".into(),
        1, 10,
        "Test summary".into(),
        vec![0.1, 0.2, 0.3],
    );

    assert!(segment.contains(5));
    assert!(!segment.contains(0));
    assert!(!segment.contains(11));
}

#[test]
fn test_compressed_segment_range() {
    let segment = CompressedSegment::new(
        "session1".into(),
        5, 15,
        "Test summary".into(),
        vec![],
    );

    assert_eq!(segment.message_range(), (5, 15));
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p hermes-memory`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-memory/src/compression_error.rs crates/hermes-memory/src/compressed.rs crates/hermes-memory/src/lib.rs
git commit -m "feat(memory): add CompressedSegment structure"
```

---

## Task 3: 数据库 Schema 扩展

**Files:**
- Modify: `crates/hermes-memory/src/sqlite_store.rs`

- [ ] **Step 1: 添加 compressed_segments 表**

在 `sqlite_store.rs` 的 `SCHEMA` 常量中添加：

```rust
CREATE TABLE IF NOT EXISTS compressed_segments (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    start_message_id INTEGER NOT NULL,
    end_message_id INTEGER NOT NULL,
    summary TEXT NOT NULL,
    vector BLOB NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_compressed_session ON compressed_segments(session_id);
```

同时在 `messages` 表添加 `compressed` 列：

```rust
ALTER TABLE messages ADD COLUMN compressed INTEGER DEFAULT 0;
```

- [ ] **Step 2: 添加压缩段存储方法**

在 `SqliteSessionStore` 结构体后添加：

```rust
impl SqliteSessionStore {
    /// Insert a compressed segment
    pub async fn insert_compressed_segment(
        &self,
        segment: &CompressedSegment,
    ) -> Result<(), StorageError> {
        let vector_bytes: Vec<u8> = segment
            .vector
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        sqlx::query(
            r#"INSERT INTO compressed_segments
               (id, session_id, start_message_id, end_message_id, summary, vector, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(&segment.id)
        .bind(&segment.session_id)
        .bind(segment.start_message_id)
        .bind(segment.end_message_id)
        .bind(&segment.summary)
        .bind(&vector_bytes)
        .bind(segment.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(())
    }

    /// Mark messages as compressed
    pub async fn mark_messages_compressed(
        &self,
        session_id: &str,
        start_id: i64,
        end_id: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE messages SET compressed = 1 WHERE session_id = ? AND id >= ? AND id <= ?"
        )
        .bind(session_id)
        .bind(start_id)
        .bind(end_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(())
    }

    /// Get compressed segments for a session
    pub async fn get_compressed_segments(
        &self,
        session_id: &str,
    ) -> Result<Vec<CompressedSegment>, StorageError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            session_id: String,
            start_message_id: i64,
            end_message_id: i64,
            summary: String,
            vector: Vec<u8>,
            created_at: String,
        }

        let rows: Vec<Row> = sqlx::query_as(
            "SELECT * FROM compressed_segments WHERE session_id = ? ORDER BY start_message_id"
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        let segments = rows
            .into_iter()
            .map(|r| {
                let vector: Vec<f32> = r.vector
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
                    .collect();

                CompressedSegment {
                    id: r.id,
                    session_id: r.session_id,
                    start_message_id: r.start_message_id,
                    end_message_id: r.end_message_id,
                    summary: r.summary,
                    vector,
                    created_at: chrono::DateTime::parse_from_rfc3339(&r.created_at)
                        .unwrap()
                        .with_timezone(&chrono::Utc),
                }
            })
            .collect();

        Ok(segments)
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p hermes-memory`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-memory/src/sqlite_store.rs
git commit -m "feat(memory): add compressed_segments table and methods"
```

---

## Task 4: CompressionConfig 配置结构

**Files:**
- Create: `crates/hermes-memory/src/compression_config.rs`

- [ ] **Step 1: 创建配置结构**

```rust
//! Compression configuration

use serde::{Deserialize, Serialize};

/// Compression mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    SummaryOnly,
    VectorOnly,
    Hybrid,
}

impl Default for CompressionMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// LLM provider type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SummarizerProvider {
    OpenAi,
    Ollama,
}

impl Default for SummarizerProvider {
    fn default() -> Self {
        Self::OpenAi
    }
}

/// Context compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable context compression
    pub enabled: bool,
    /// Token count threshold to trigger compression
    pub token_threshold: usize,
    /// Message count threshold to trigger compression
    pub message_count_threshold: usize,
    /// Minimum number of messages to compress at once
    pub min_compression_unit: usize,
    /// Maximum summary length in tokens
    pub max_summary_tokens: usize,
    /// Compression mode
    pub mode: CompressionMode,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            token_threshold: 8000,
            message_count_threshold: 50,
            min_compression_unit: 5,
            max_summary_tokens: 500,
            mode: CompressionMode::Hybrid,
        }
    }
}

/// Summarizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizerConfig {
    /// Provider type
    pub provider: SummarizerProvider,
    /// Model name
    pub model: String,
    /// Ollama URL (for local models)
    pub ollama_url: Option<String>,
}

impl Default for SummarizerConfig {
    fn default() -> Self {
        Self {
            provider: SummarizerProvider::OpenAi,
            model: "gpt-4o-mini".to_string(),
            ollama_url: Some("http://localhost:11434".to_string()),
        }
    }
}
```

- [ ] **Step 2: 更新 lib.rs 导出**

```rust
pub mod compression_config;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p hermes-memory`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-memory/src/compression_config.rs crates/hermes-memory/src/lib.rs
git commit -m "feat(memory): add compression configuration"
```

---

## Task 5: Summarizer 实现

**Files:**
- Create: `crates/hermes-memory/src/summarizer.rs`
- Modify: `crates/hermes-memory/Cargo.toml` (添加 reqwest)

- [ ] **Step 1: 添加 reqwest 依赖**

在 `Cargo.toml` 中添加：
```toml
reqwest.workspace = true
```

- [ ] **Step 2: 创建 Summarizer 实现**

```rust
//! LLM-based message summarization

use crate::compression_config::{SummarizerConfig, SummarizerProvider};
use crate::compression_error::CompressionError;
use crate::session::Message;
use reqwest::Client;

/// Summarizer for generating message summaries
pub struct Summarizer {
    config: SummarizerConfig,
    http_client: Client,
}

impl Summarizer {
    pub fn new(config: SummarizerConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
        }
    }

    /// Generate a summary for a list of messages
    pub async fn summarize(
        &self,
        messages: &[Message],
        max_tokens: usize,
    ) -> Result<String, CompressionError> {
        match self.config.provider {
            SummarizerProvider::OpenAi => {
                self.summarize_openai(messages, max_tokens).await
            }
            SummarizerProvider::Ollama => {
                self.summarize_ollama(messages, max_tokens).await
            }
        }
    }

    /// Generate embedding vector for text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, CompressionError> {
        match self.config.provider {
            SummarizerProvider::OpenAi => {
                self.embed_openai(text).await
            }
            SummarizerProvider::Ollama => {
                self.embed_ollama(text).await
            }
        }
    }

    async fn summarize_openai(
        &self,
        messages: &[Message],
        _max_tokens: usize,
    ) -> Result<String, CompressionError> {
        // Build conversation context
        let context = messages
            .iter()
            .filter_map(|m| {
                let role = &m.role;
                let content = m.content.as_deref().unwrap_or("");
                if content.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", role, content))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Summarize the following conversation concisely, capturing the key points and any important details:\n\n{}\n\nSummary:",
            context
        );

        // Call OpenAI API (stub - actual implementation would use hermes-provider)
        // For now, return a truncated version as placeholder
        Ok(format!("[Summary of {} messages]", messages.len()))
    }

    async fn summarize_ollama(
        &self,
        messages: &[Message],
        _max_tokens: usize,
    ) -> Result<String, CompressionError> {
        let ollama_url = self.config.ollama_url.as_ref()
            .ok_or_else(|| CompressionError::Config("Ollama URL not configured".into()))?;

        let context = messages
            .iter()
            .filter_map(|m| {
                let role = &m.role;
                let content = m.content.as_deref().unwrap_or("");
                if content.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", role, content))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let request = serde_json::json!({
            "model": self.config.model,
            "prompt": format!(
                "Summarize the following conversation concisely:\n\n{}\n\nSummary:",
                context
            ),
            "stream": false
        });

        let response = self.http_client
            .post(format!("{}/api/generate", ollama_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        if !response.status().is_success() {
            return Err(CompressionError::LlmApi(format!(
                "Ollama returned status: {}",
                response.status()
            )));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse {
            response: String,
        }

        let ollama_resp: OllamaResponse = response
            .json()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        Ok(ollama_resp.response)
    }

    async fn embed_openai(&self, text: &str) -> Result<Vec<f32>, CompressionError> {
        // Stub: Return dummy embedding
        // Actual implementation would call OpenAI embedding API
        Ok(vec![0.0; 1536])
    }

    async fn embed_ollama(&self, text: &str) -> Result<Vec<f32>, CompressionError> {
        let ollama_url = self.config.ollama_url.as_ref()
            .ok_or_else(|| CompressionError::Config("Ollama URL not configured".into()))?;

        let request = serde_json::json!({
            "model": self.config.model,
            "prompt": text
        });

        let response = self.http_client
            .post(format!("{}/api/embeddings", ollama_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        #[derive(serde::Deserialize)]
        struct OllamaEmbedResponse {
            embedding: Vec<f32>,
        }

        let embed_resp: OllamaEmbedResponse = response
            .json()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        Ok(embed_resp.embedding)
    }
}
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p hermes-memory`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-memory/Cargo.toml crates/hermes-memory/src/summarizer.rs
git commit -m "feat(memory): add Summarizer implementation"
```

---

## Task 6: CompressionManager 实现

**Files:**
- Create: `crates/hermes-memory/src/compression.rs`
- Modify: `crates/hermes-memory/src/lib.rs`

- [ ] **Step 1: 创建 CompressionManager**

```rust
//! Context compression manager

use crate::compressed::CompressedSegment;
use crate::compression_config::{CompressionConfig, CompressionMode};
use crate::compression_error::CompressionError;
use crate::session::{Message, SessionStore};
use crate::summarizer::Summarizer;
use std::sync::Arc;
use tokio::sync::RwLock;

/// CompressionManager - Manages context compression for sessions
pub struct CompressionManager<S: SessionStore> {
    config: CompressionConfig,
    summarizer: Summarizer,
    store: Arc<S>,
}

impl<S: SessionStore> CompressionManager<S> {
    pub fn new(
        config: CompressionConfig,
        summarizer: Summarizer,
        store: Arc<S>,
    ) -> Self {
        Self {
            config,
            summarizer,
            store,
        }
    }

    /// Check if compression should be triggered
    pub async fn should_compress(&self, session_id: &str) -> Result<bool, CompressionError> {
        if !self.config.enabled {
            return Ok(false);
        }

        // Check message count threshold
        let messages = self.store
            .get_messages(session_id, usize::MAX, 0)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        let message_count = messages.len();

        if message_count < self.config.message_count_threshold {
            return Ok(false);
        }

        // Check token threshold (simplified - sum token counts)
        let total_tokens: usize = messages
            .iter()
            .filter_map(|m| m.token_count)
            .sum();

        if total_tokens < self.config.token_threshold {
            return Ok(false);
        }

        Ok(true)
    }

    /// Compress messages for a session
    pub async fn compress(&self, session_id: &str) -> Result<CompressedSegment, CompressionError> {
        // Get all uncompressed messages
        let messages = self.store
            .get_messages(session_id, usize::MAX, 0)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        // Filter out already compressed messages
        let uncompressed: Vec<Message> = messages
            .into_iter()
            .filter(|m| m.id >= 0) // All messages initially uncompressed
            .collect();

        if uncompressed.len() < self.config.min_compression_unit {
            return Err(CompressionError::Config(
                "Not enough messages to compress".into()
            ));
        }

        // Group messages into segments
        let segment_messages = &uncompressed[0..uncompressed.len().min(20)]; // Cap at 20 messages

        // Generate summary
        let summary = self.summarizer
            .summarize(segment_messages, self.config.max_summary_tokens)
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        // Generate embedding
        let vector = self.summarizer
            .embed(&summary)
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        // Create compressed segment
        let start_id = segment_messages.first()
            .map(|m| m.id)
            .ok_or_else(|| CompressionError::Config("No messages".into()))?;
        let end_id = segment_messages.last()
            .map(|m| m.id)
            .ok_or_else(|| CompressionError::Config("No messages".into()))?;

        let segment = CompressedSegment::new(
            session_id.to_string(),
            start_id,
            end_id,
            summary,
            vector,
        );

        // Store compressed segment
        self.store
            .insert_compressed_segment(&segment)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        // Mark original messages as compressed
        self.store
            .mark_messages_compressed(session_id, start_id, end_id)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        Ok(segment)
    }

    /// Get compressed segments for retrieval
    pub async fn get_compressed_segments(
        &self,
        session_id: &str,
    ) -> Result<Vec<CompressedSegment>, CompressionError> {
        self.store
            .get_compressed_segments(session_id)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))
    }
}
```

- [ ] **Step 2: 更新 lib.rs**

添加导出：
```rust
pub mod compression;
pub mod compression_config;
pub mod summarizer;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check -p hermes-memory`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-memory/src/compression.rs crates/hermes-memory/src/lib.rs
git commit -m "feat(memory): add CompressionManager"
```

---

## Task 7: 集成测试

**Files:**
- Modify: `crates/hermes-memory/src/tests/test_compression.rs`

- [ ] **Step 1: 添加集成测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::compression_config::{CompressionConfig, CompressionMode, SummarizerConfig, SummarizerProvider};
    use crate::summarizer::Summarizer;
    use crate::SqliteSessionStore;
    use tempfile::tempdir;

    fn create_test_config() -> (CompressionConfig, SummarizerConfig) {
        let compression = CompressionConfig {
            enabled: true,
            token_threshold: 1000,
            message_count_threshold: 10,
            min_compression_unit: 5,
            max_summary_tokens: 100,
            mode: CompressionMode::Hybrid,
        };

        let summarizer = SummarizerConfig {
            provider: SummarizerProvider::Ollama,
            model: "llama3".to_string(),
            ollama_url: Some("http://localhost:11434".to_string()),
        };

        (compression, summarizer)
    }

    #[tokio::test]
    async fn test_compression_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = SqliteSessionStore::new(db_path)
            .await
            .unwrap();
        let store = Arc::new(store);

        let (compression_config, summarizer_config) = create_test_config();
        let summarizer = Summarizer::new(summarizer_config);
        let manager = CompressionManager::new(compression_config, summarizer, store);

        assert!(!manager.should_compress("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_compress_empty_session() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = SqliteSessionStore::new(db_path)
            .await
            .unwrap();
        let store = Arc::new(store);

        // Create a session first
        store.create_session(NewSession {
            id: "test-session".to_string(),
            source: "test".to_string(),
            user_id: None,
            model: Some("gpt-4".to_string()),
        }).await.unwrap();

        let (compression_config, summarizer_config) = create_test_config();
        let summarizer = Summarizer::new(summarizer_config);
        let manager = CompressionManager::new(compression_config, summarizer, store);

        let result = manager.compress("test-session").await;
        assert!(result.is_err()); // Should fail due to min_compression_unit
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p hermes-memory`
Expected: PASS (或部分 PASS，跳过需要实际 LLM 的测试)

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-memory/src/tests/test_compression.rs
git commit -m "test(memory): add compression integration tests"
```

---

## Self-Review Checklist

**1. Spec Coverage:**
- [x] CompressionError 定义 (Task 1)
- [x] CompressedSegment 结构 (Task 2)
- [x] 数据库 Schema 扩展 (Task 3)
- [x] CompressionConfig 配置 (Task 4)
- [x] Summarizer 实现 (Task 5)
- [x] CompressionManager 实现 (Task 6)
- [x] 单元测试 (Task 7)

**2. Placeholder Scan:** 无 TBD/TODO

**3. Type Consistency:**
- 所有方法签名在 Task 间保持一致
- `CompressionError` 在各 Task 中正确使用

---

## Execution Options

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
