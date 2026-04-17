//! MemoryTool — 跨会话持久化记忆工具
//!
//! 提供 K-V 存储，支持 set / get / search / read 四种操作。
//! 支持 category 和 tags 字段，并使用 SQLite FTS5 进行全文搜索。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_memory::SqliteSessionStore;
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// MemoryTool — 跨会话持久化记忆工具
#[derive(Clone)]
pub struct MemoryTool {
    store: Arc<SqliteSessionStore>,
}

impl std::fmt::Debug for MemoryTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryTool").finish()
    }
}

impl MemoryTool {
    pub fn new(store: Arc<SqliteSessionStore>) -> Self {
        Self { store }
    }

    /// Initialize FTS5 table and triggers, migrate schema if needed
    pub async fn ensure_fts(&self) -> Result<(), ToolError> {
        let pool = self.store.pool();

        // ALTER TABLE to add category and tags columns if not exist
        // Only ignore "duplicate column" error — let other errors propagate
        if let Err(e) = sqlx::query("ALTER TABLE memory ADD COLUMN category TEXT")
            .execute(pool)
            .await
        {
            let is_column_exists = e.to_string().contains("duplicate column");
            if !is_column_exists {
                return Err(ToolError::Execution(format!("ALTER TABLE error: {}", e)));
            }
        }

        if let Err(e) = sqlx::query("ALTER TABLE memory ADD COLUMN tags TEXT")
            .execute(pool)
            .await
        {
            let is_column_exists = e.to_string().contains("duplicate column");
            if !is_column_exists {
                return Err(ToolError::Execution(format!("ALTER TABLE error: {}", e)));
            }
        }

        // Create FTS5 virtual table
        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(key, value, category, content=memory, content_rowid=rowid)"
        )
        .execute(pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS table creation error: {}", e)))?;

        // Create insert trigger
        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS memory_fts_insert AFTER INSERT ON memory BEGIN INSERT INTO memory_fts(rowid, key, value, category) VALUES (new.rowid, new.key, new.value, new.category); END"
        )
        .execute(pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS insert trigger error: {}", e)))?;

        // Create delete trigger
        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS memory_fts_delete AFTER DELETE ON memory BEGIN INSERT INTO memory_fts(memory_fts, rowid, key, value, category) VALUES('delete', old.rowid, old.key, old.value, old.category); END"
        )
        .execute(pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS delete trigger error: {}", e)))?;

        // Create update trigger
        sqlx::query(
            "CREATE TRIGGER IF NOT EXISTS memory_fts_update AFTER UPDATE ON memory BEGIN INSERT INTO memory_fts(memory_fts, rowid, key, value, category) VALUES('delete', old.rowid, old.key, old.value, old.category); INSERT INTO memory_fts(rowid, key, value, category) VALUES (new.rowid, new.key, new.value, new.category); END"
        )
        .execute(pool)
        .await
        .map_err(|e| ToolError::Execution(format!("FTS update trigger error: {}", e)))?;

        // Populate FTS table with existing data (handles rows added before FTS existed)
        let existing_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM memory_fts")
            .fetch_one(pool)
            .await
            .map_err(|e| ToolError::Execution(format!("FTS count error: {}", e)))?;

        if existing_count.0 == 0 {
            // Table is empty, populate from memory table
            sqlx::query(
                "INSERT INTO memory_fts(rowid, key, value, category) SELECT rowid, key, value, category FROM memory"
            )
            .execute(pool)
            .await
            .map_err(|e| ToolError::Execution(format!("FTS population error: {}", e)))?;
        }

        Ok(())
    }

    /// Search using FTS5
    pub async fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryResult>, ToolError> {
        let pattern = format!("\"{}\"", query.replace('"', "\"\""));
        let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT m.key, m.value, m.category FROM memory m JOIN memory_fts f ON m.rowid = f.rowid WHERE memory_fts MATCH ? LIMIT ?"
        )
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(self.store.pool())
        .await
        .map_err(|e| ToolError::Execution(format!("FTS search error: {}", e)))?;

        Ok(rows.into_iter().map(|(k, v, c)| MemoryResult { key: k, value: v, category: c }).collect())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct MemoryResult {
    pub key: String,
    pub value: String,
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum MemoryParams {
    Set {
        key: String,
        value: String,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
    },
    Get { key: String },
    Search { query: String, #[serde(default)] limit: Option<usize> },
    Read { #[serde(default)] category: Option<String> },
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &str { "memory" }

    fn description(&self) -> &str {
        "Cross-session persistent memory. Supports set(key, value, category?, tags?), get(key), search(query), read(category?)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "action": { "const": "set" },
                        "key": { "type": "string" },
                        "value": { "type": "string" },
                        "category": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } }
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
                },
                {
                    "properties": {
                        "action": { "const": "read" },
                        "category": { "type": "string" }
                    },
                    "required": ["action"]
                }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        // Ensure FTS is initialized
        self.ensure_fts().await?;

        let params: MemoryParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params {
            MemoryParams::Set { key, value, category, tags } => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as f64)
                    .unwrap_or(0.0);
                let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());

                // Check if entry exists to preserve created_at
                let existing: Option<(f64,)> = sqlx::query_as(
                    "SELECT created_at FROM memory WHERE key = ?"
                )
                .bind(&key)
                .fetch_optional(self.store.pool())
                .await
                .map_err(|e| ToolError::Execution(format!("Memory set error: {}", e)))?;

                let created_at = existing.map(|(t,)| t).unwrap_or(now);

                sqlx::query(
                    "INSERT OR REPLACE INTO memory (key, value, category, tags, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(&key)
                .bind(&value)
                .bind(&category)
                .bind(&tags_json)
                .bind(created_at)
                .bind(now)
                .execute(self.store.pool())
                .await
                .map_err(|e| ToolError::Execution(format!("Memory set error: {}", e)))?;

                let mut result = json!({ "status": "ok", "key": key });
                if let Some(cat) = category {
                    result["category"] = json!(cat);
                }
                Ok(result.to_string())
            }
            MemoryParams::Get { key } => {
                let row: Option<(String, Option<String>)> = sqlx::query_as(
                    "SELECT value, category FROM memory WHERE key = ?"
                )
                .bind(&key)
                .fetch_optional(self.store.pool())
                .await
                .map_err(|e| ToolError::Execution(format!("Memory get error: {}", e)))?;

                match row {
                    Some((value, category)) => {
                        let mut result = json!({ "key": key, "value": value });
                        if let Some(cat) = category {
                            result["category"] = json!(cat);
                        }
                        Ok(result.to_string())
                    }
                    None => Ok(json!({ "key": key, "value": null }).to_string()),
                }
            }
            MemoryParams::Search { query, limit } => {
                let limit = limit.unwrap_or(5);
                let results = self.search_fts(&query, limit).await?;
                let json_results: Vec<_> = results.into_iter().map(|r| {
                    let mut obj = json!({"key": r.key, "value": r.value});
                    if let Some(cat) = r.category {
                        obj["category"] = json!(cat);
                    }
                    obj
                }).collect();
                Ok(json!({ "results": json_results }).to_string())
            }
            MemoryParams::Read { category } => {
                let rows: Vec<(String, String, Option<String>)> = match &category {
                    Some(cat) => {
                        sqlx::query_as(
                            "SELECT key, value, category FROM memory WHERE category = ? ORDER BY updated_at DESC"
                        )
                        .bind(cat)
                        .fetch_all(self.store.pool())
                        .await
                        .map_err(|e| ToolError::Execution(format!("Memory read error: {}", e)))?
                    }
                    None => {
                        sqlx::query_as(
                            "SELECT key, value, category FROM memory ORDER BY updated_at DESC"
                        )
                        .fetch_all(self.store.pool())
                        .await
                        .map_err(|e| ToolError::Execution(format!("Memory read error: {}", e)))?
                    }
                };

                let results: Vec<_> = rows.into_iter().map(|(k, v, c)| {
                    let mut obj = json!({"key": k, "value": v});
                    if let Some(cat) = c {
                        obj["category"] = json!(cat);
                    }
                    obj
                }).collect();

                Ok(json!({ "results": results }).to_string())
            }
        }
    }
}
