//! SQLite 会话存储实现
//!
//! 本模块实现了 `hermes-memory` 中定义的 `SessionStore` trait，使用 SQLite 作为持久化后端。
//!
//! ## 数据库架构
//!
//! 数据库包含以下表：
//!
//! - **`sessions`** — 存储会话元数据。每个会话有一个唯一 ID、来源（source）、
//!   使用的模型、token 统计、计费信息、结束原因等。sessions 表上还建立了
//!   `source`、`parent_session_id`、`started_at` 的索引以加速常见查询。
//!
//! - **`messages`** — 存储会话中的每条消息。每条消息关联到一个 session_id，
//!   包含角色（user/assistant）、内容、工具调用信息和时间戳。通过
//!   `(session_id, timestamp)` 复合索引加速按时间顺序获取消息。
//!
//! - **`messages_fts`** — FTS5 虚拟表，提供消息内容的全文搜索能力。
//!   通过触发器与 `messages` 表保持同步。当插入新消息时，自动索引其内容。
//!
//! ## SqliteSessionStore
//!
//! 核心结构体 `SqliteSessionStore` 持有 `SqlitePool` 连接池，并实现了以下方法：
//!
//! - **`new(db_path)`** — 异步构造方法。创建连接池并执行数据库迁移（创建表和索引）。
//!
//! ## DB 行类型转换
//!
//! 模块定义了三个内部 DB 行类型（`SessionDbRow`、`MessageDbRow`、`SearchDbRow`）
//! 以及对应的 `From` 实现，用于将 sqlx 查询结果转换为公共领域类型。
//!
//! ## 错误处理
//!
//! 所有数据库操作返回 `StorageError`，区分三类错误来源：
//! - `Connection` — 无法建立数据库连接
//! - `Migration` — schema 初始化失败
//! - `Query` — SQL 查询执行失败

use crate::{Message, NewMessage, NewSession, SearchResult, Session, SessionStore, compressed::CompressedSegment};
use async_trait::async_trait;
use hermes_error::StorageError;
use sqlx::{SqlitePool, FromRow};
use std::path::PathBuf;
use std::time::SystemTime;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    user_id TEXT,
    model TEXT,
    model_config TEXT,
    system_prompt TEXT,
    parent_session_id TEXT,
    started_at REAL NOT NULL,
    ended_at REAL,
    end_reason TEXT,
    message_count INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_write_tokens INTEGER DEFAULT 0,
    reasoning_tokens INTEGER DEFAULT 0,
    billing_provider TEXT,
    billing_base_url TEXT,
    billing_mode TEXT,
    estimated_cost_usd REAL,
    actual_cost_usd REAL,
    title TEXT,
    metadata TEXT
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT,
    tool_call_id TEXT,
    tool_calls TEXT,
    tool_name TEXT,
    timestamp REAL NOT NULL,
    compressed INTEGER DEFAULT 0,
    token_count INTEGER,
    finish_reason TEXT,
    reasoning TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

CREATE INDEX IF NOT EXISTS idx_sessions_source ON sessions(source);
CREATE INDEX IF NOT EXISTS idx_sessions_parent ON sessions(parent_session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_started ON sessions(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, timestamp);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content=messages,
    content_rowid=id
);

CREATE TRIGGER IF NOT EXISTS messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TABLE IF NOT EXISTS memory (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at REAL NOT NULL,
    updated_at REAL NOT NULL
);

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
"#;

pub struct SqliteSessionStore {
    pool: SqlitePool,
}

impl SqliteSessionStore {
    pub async fn new(db_path: PathBuf) -> Result<Self, StorageError> {
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&database_url)
            .await
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        sqlx::query(SCHEMA)
            .execute(&pool)
            .await
            .map_err(|e| StorageError::Migration(e.to_string()))?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[derive(Debug, FromRow)]
struct SessionDbRow {
    id: String,
    source: String,
    user_id: Option<String>,
    model: Option<String>,
    model_config: Option<String>,
    system_prompt: Option<String>,
    parent_session_id: Option<String>,
    started_at: f64,
    ended_at: Option<f64>,
    end_reason: Option<String>,
    message_count: i64,
    tool_call_count: i64,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    reasoning_tokens: i64,
    billing_provider: Option<String>,
    billing_base_url: Option<String>,
    billing_mode: Option<String>,
    estimated_cost_usd: Option<f64>,
    actual_cost_usd: Option<f64>,
    title: Option<String>,
    metadata: Option<String>,
}

impl From<SessionDbRow> for Session {
    fn from(r: SessionDbRow) -> Self {
        Session {
            id: r.id,
            source: r.source,
            user_id: r.user_id,
            model: r.model,
            model_config: r.model_config,
            system_prompt: r.system_prompt,
            parent_session_id: r.parent_session_id,
            started_at: r.started_at,
            ended_at: r.ended_at,
            end_reason: r.end_reason,
            message_count: r.message_count as usize,
            tool_call_count: r.tool_call_count as usize,
            input_tokens: r.input_tokens as usize,
            output_tokens: r.output_tokens as usize,
            cache_read_tokens: r.cache_read_tokens as usize,
            cache_write_tokens: r.cache_write_tokens as usize,
            reasoning_tokens: r.reasoning_tokens as usize,
            billing_provider: r.billing_provider,
            billing_base_url: r.billing_base_url,
            billing_mode: r.billing_mode,
            estimated_cost_usd: r.estimated_cost_usd,
            actual_cost_usd: r.actual_cost_usd,
            title: r.title,
            metadata: r.metadata,
        }
    }
}

#[derive(Debug, FromRow)]
struct MessageDbRow {
    id: i64,
    session_id: String,
    role: String,
    content: Option<String>,
    tool_call_id: Option<String>,
    tool_calls: Option<String>,
    tool_name: Option<String>,
    timestamp: f64,
    token_count: Option<i64>,
    finish_reason: Option<String>,
    reasoning: Option<String>,
}

impl From<MessageDbRow> for Message {
    fn from(r: MessageDbRow) -> Self {
        Message {
            id: r.id,
            session_id: r.session_id,
            role: r.role,
            content: r.content,
            tool_call_id: r.tool_call_id,
            tool_calls: r.tool_calls,
            tool_name: r.tool_name,
            timestamp: r.timestamp,
            token_count: r.token_count.map(|v| v as usize),
            finish_reason: r.finish_reason,
            reasoning: r.reasoning,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct SearchDbRow {
    id: i64,
    session_id: String,
    content: Option<String>,
    snippet: String,
}

impl From<SearchDbRow> for SearchResult {
    fn from(r: SearchDbRow) -> Self {
        SearchResult {
            id: r.id,
            session_id: r.session_id,
            content: r.content.unwrap_or_default(),
            snippet: r.snippet,
        }
    }
}

#[async_trait]
impl SessionStore for SqliteSessionStore {
    async fn create_session(&self, session: NewSession) -> Result<Session, StorageError> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        sqlx::query(
            r#"INSERT INTO sessions (id, source, user_id, model, started_at, message_count, tool_call_count)
               VALUES (?, ?, ?, ?, ?, 0, 0)"#,
        )
        .bind(&session.id)
        .bind(&session.source)
        .bind(&session.user_id)
        .bind(&session.model)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(Session {
            id: session.id,
            source: session.source,
            user_id: session.user_id,
            model: session.model,
            model_config: None,
            system_prompt: None,
            parent_session_id: None,
            started_at: now,
            ended_at: None,
            end_reason: None,
            message_count: 0,
            tool_call_count: 0,
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            reasoning_tokens: 0,
            billing_provider: None,
            billing_base_url: None,
            billing_mode: None,
            estimated_cost_usd: None,
            actual_cost_usd: None,
            title: None,
            metadata: None,
        })
    }

    async fn get_session(&self, session_id: &str) -> Result<Option<Session>, StorageError> {
        let row: Option<SessionDbRow> = sqlx::query_as(
            "SELECT * FROM sessions WHERE id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(row.map(|r| r.into()))
    }

    async fn append_message(
        &self,
        session_id: &str,
        message: NewMessage,
    ) -> Result<Message, StorageError> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        let id: i64 = sqlx::query_scalar(
            r#"INSERT INTO messages (session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, reasoning)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               RETURNING id"#,
        )
        .bind(session_id)
        .bind(&message.role)
        .bind(&message.content)
        .bind(&message.tool_call_id)
        .bind(&message.tool_calls)
        .bind(&message.tool_name)
        .bind(timestamp)
        .bind(message.token_count.map(|v| v as i64))
        .bind(&message.finish_reason)
        .bind(&message.reasoning)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        // Update message count
        sqlx::query("UPDATE sessions SET message_count = message_count + 1 WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(Message {
            id,
            session_id: session_id.to_string(),
            role: message.role,
            content: message.content,
            tool_call_id: message.tool_call_id,
            tool_calls: message.tool_calls,
            tool_name: message.tool_name,
            timestamp,
            token_count: message.token_count,
            finish_reason: message.finish_reason,
            reasoning: message.reasoning,
        })
    }

    async fn get_messages(
        &self,
        session_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Message>, StorageError> {
        let rows: Vec<MessageDbRow> = sqlx::query_as(
            "SELECT * FROM messages WHERE session_id = ? ORDER BY timestamp LIMIT ? OFFSET ?"
        )
        .bind(session_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn search_messages(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, StorageError> {
        let rows: Vec<SearchDbRow> = sqlx::query_as(
            r#"SELECT m.id, m.session_id, m.content,
                      snippet('messages_fts', 0, '<mark>', '</mark>', '...', 32) as snippet
               FROM messages_fts
               JOIN messages m ON messages_fts.rowid = m.id
               WHERE messages_fts MATCH ?
               ORDER BY rank
               LIMIT ?"#
        )
        .bind(query)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn list_sessions(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Session>, StorageError> {
        let rows: Vec<SessionDbRow> = sqlx::query_as(
            "SELECT * FROM sessions ORDER BY started_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn delete_session(&self, session_id: &str) -> Result<(), StorageError> {
        // 先删除压缩段落（引用 messages 表的外键），再删除消息，最后删除会话
        // Delete compressed segments first (they reference messages), then messages, then session
        sqlx::query("DELETE FROM compressed_segments WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }

    async fn update_session(&self, session: &Session) -> Result<(), StorageError> {
        sqlx::query(
            r#"UPDATE sessions SET 
               source = ?, user_id = ?, model = ?, model_config = ?, system_prompt = ?, 
               parent_session_id = ?, ended_at = ?, end_reason = ?, 
               message_count = ?, tool_call_count = ?, 
               input_tokens = ?, output_tokens = ?, cache_read_tokens = ?, 
               cache_write_tokens = ?, reasoning_tokens = ?, 
               billing_provider = ?, billing_base_url = ?, billing_mode = ?, 
               estimated_cost_usd = ?, actual_cost_usd = ?, 
               title = ?, metadata = ? 
               WHERE id = ?"#
        )
        .bind(&session.source)
        .bind(&session.user_id)
        .bind(&session.model)
        .bind(&session.model_config)
        .bind(&session.system_prompt)
        .bind(&session.parent_session_id)
        .bind(&session.ended_at)
        .bind(&session.end_reason)
        .bind(session.message_count as i64)
        .bind(session.tool_call_count as i64)
        .bind(session.input_tokens as i64)
        .bind(session.output_tokens as i64)
        .bind(session.cache_read_tokens as i64)
        .bind(session.cache_write_tokens as i64)
        .bind(session.reasoning_tokens as i64)
        .bind(&session.billing_provider)
        .bind(&session.billing_base_url)
        .bind(&session.billing_mode)
        .bind(&session.estimated_cost_usd)
        .bind(&session.actual_cost_usd)
        .bind(&session.title)
        .bind(&session.metadata)
        .bind(&session.id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }

    async fn get_session_stats(&self, session_id: &str) -> Result<Option<(usize, usize, usize)>, StorageError> {
        let row: Option<(i64, i64, i64)> = sqlx::query_as(
            "SELECT message_count, input_tokens, output_tokens FROM sessions WHERE id = ?"
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(row.map(|(msg_count, input_tokens, output_tokens)| (
            msg_count as usize,
            input_tokens as usize,
            output_tokens as usize
        )))
    }

    async fn search_sessions_by_model(&self, model: &str, limit: usize) -> Result<Vec<Session>, StorageError> {
        let rows: Vec<SessionDbRow> = sqlx::query_as(
            "SELECT * FROM sessions WHERE model LIKE ? ORDER BY started_at DESC LIMIT ?"
        )
        .bind(format!("%{model}%"))
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn insert_compressed_segment(&self, segment: &CompressedSegment) -> Result<(), StorageError> {
        SqliteSessionStore::insert_compressed_segment(self, segment).await
    }

    async fn mark_messages_compressed(&self, session_id: &str, start_id: i64, end_id: i64) -> Result<(), StorageError> {
        SqliteSessionStore::mark_messages_compressed(self, session_id, start_id, end_id).await
    }

    async fn get_compressed_segments(&self, session_id: &str) -> Result<Vec<CompressedSegment>, StorageError> {
        SqliteSessionStore::get_compressed_segments(self, session_id).await
    }
}

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

        let segments: Vec<CompressedSegment> = rows
            .into_iter()
            .map(|r| {
                let vector: Vec<f32> = r.vector
                    .chunks_exact(4)
                    .map(|chunk| {
                        let bytes: [u8; 4] = chunk.try_into()
                            .map_err(|e| StorageError::Query(format!("Invalid f32 bytes: {}", e)))?;
                        Ok(f32::from_le_bytes(bytes))
                    })
                    .collect::<Result<Vec<f32>, StorageError>>()
                    .map_err(|e| StorageError::Query(format!("Invalid vector data: {}", e)))?;

                let created_at = chrono::DateTime::parse_from_rfc3339(&r.created_at)
                    .map_err(|e| StorageError::Query(format!("Invalid date format: {}", e)))?
                    .with_timezone(&chrono::Utc);

                Ok(CompressedSegment {
                    id: r.id,
                    session_id: r.session_id,
                    start_message_id: r.start_message_id,
                    end_message_id: r.end_message_id,
                    summary: r.summary,
                    vector,
                    created_at,
                })
            })
            .collect::<Result<Vec<CompressedSegment>, StorageError>>()?;

        Ok(segments)
    }
}
