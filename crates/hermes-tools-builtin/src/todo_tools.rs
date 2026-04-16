//! todo_tools — 任务列表管理工具
//!
//! 提供会话内任务管理，支持 replace 和 merge 两种写入模式。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// 有效的任务状态
const VALID_STATUSES: &[&str] = &["pending", "in_progress", "completed", "cancelled"];

/// 单个任务项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: String,
}

/// 任务统计摘要
#[derive(Debug, Clone, Serialize)]
pub struct TodoSummary {
    pub total: usize,
    pub pending: usize,
    pub in_progress: usize,
    pub completed: usize,
    pub cancelled: usize,
}

/// 任务存储（内存）
#[derive(Debug, Default)]
pub struct TodoStore {
    items: Vec<TodoItem>,
}

impl TodoStore {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// 写入任务列表
    ///
    /// `merge=false`: 替换整个列表
    /// `merge=true`: 按 id 合并更新
    pub fn write(&mut self, todos: Vec<TodoItem>, merge: bool) -> Vec<TodoItem> {
        if !merge {
            self.items = Self::dedupe_by_id(self.validate_all(todos));
        } else {
            let validated = self.validate_all(todos);
            let existing: std::collections::HashMap<String, usize> = self.items
                .iter()
                .enumerate()
                .map(|(i, t)| (t.id.clone(), i))
                .collect();
            for item in validated {
                if let Some(idx) = existing.get(&item.id) {
                    self.items[*idx] = item;
                } else {
                    self.items.push(item);
                }
            }
        }
        self.read()
    }

    /// 读取当前列表
    pub fn read(&self) -> Vec<TodoItem> {
        self.items.clone()
    }

    /// 返回统计摘要
    pub fn summary(&self) -> TodoSummary {
        let mut s = TodoSummary {
            total: self.items.len(),
            pending: 0,
            in_progress: 0,
            completed: 0,
            cancelled: 0,
        };
        for item in &self.items {
            match item.status.as_str() {
                "pending" => s.pending += 1,
                "in_progress" => s.in_progress += 1,
                "completed" => s.completed += 1,
                "cancelled" => s.cancelled += 1,
                _ => {}
            }
        }
        s
    }

    /// 验证并规范化所有项
    fn validate_all(&self, todos: Vec<TodoItem>) -> Vec<TodoItem> {
        todos.into_iter().map(|t| self.validate(t)).collect()
    }

    /// 验证单个项
    fn validate(&self, mut item: TodoItem) -> TodoItem {
        if item.id.trim().is_empty() {
            item.id = "?".to_string();
        }
        if item.content.trim().is_empty() {
            item.content = "(no description)".to_string();
        }
        if !VALID_STATUSES.contains(&item.status.as_str()) {
            item.status = "pending".to_string();
        }
        item
    }

    /// 按 id 去重，保留最后一次出现
    fn dedupe_by_id(todos: Vec<TodoItem>) -> Vec<TodoItem> {
        let mut last_index: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, t) in todos.iter().enumerate() {
            last_index.insert(t.id.clone(), i);
        }
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for t in &todos {
            if let Some(&idx) = last_index.get(&t.id) {
                if !seen.contains(&t.id) {
                    seen.insert(t.id.clone());
                    result.push(todos[idx].clone());
                }
            }
        }
        result
    }
}

/// TodoTool — 任务列表管理工具
pub struct TodoTool {
    store: Arc<RwLock<TodoStore>>,
}

impl TodoTool {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(TodoStore::new())),
        }
    }
}

impl Default for TodoTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TodoTool {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}

impl std::fmt::Debug for TodoTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TodoTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoParams {
    #[serde(default)]
    pub todos: Option<Vec<TodoItem>>,
    #[serde(default)]
    pub merge: Option<bool>,
}

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage your task list for the current session. Call with no parameters to read."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "Task items to write. Omit to read current list.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"] }
                        },
                        "required": ["id", "content", "status"]
                    }
                },
                "merge": {
                    "type": "boolean",
                    "description": "true: update by id. false (default): replace entire list.",
                    "default": false
                }
            }
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let params: TodoParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let merge = params.merge.unwrap_or(false);
        let mut store = self.store.write();
        let items = if let Some(todos) = params.todos {
            store.write(todos, merge)
        } else {
            store.read()
        };

        let summary = store.summary();
        Ok(json!({ "todos": items, "summary": summary }).to_string())
    }
}
