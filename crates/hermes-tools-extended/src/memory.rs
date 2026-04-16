//! MemoryTool — 跨会话持久化记忆工具
//!
//! 提供 K-V 存储，支持 set / get / search 三种操作。

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
