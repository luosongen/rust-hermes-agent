# TodoTool + ClarifyTool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现两个内置工具 — TodoTool（任务管理）和 ClarifyTool（用户交互）

**Architecture:**
- TodoTool 持有 `Arc<RwLock<TodoStore>>`，纯内存实现，支持 merge 模式和 replace 模式
- ClarifyTool 持有 `Arc<dyn Fn(...) -> String + Send + Sync>` 回调，平台层注入

**Tech Stack:** Rust async/await, async_trait, hermes-core, hermes-tool-registry

---

## 文件结构

```
crates/hermes-tools-builtin/src/
├── lib.rs              # 模块导出 + register_builtin_tools 更新
├── todo_tools.rs       # TodoStore + TodoTool（新建）
└── clarify_tools.rs    # ClarifyTool（新建）
```

---

## Task 1: TodoTool 核心实现

**Files:**
- Create: `crates/hermes-tools-builtin/src/todo_tools.rs`
- Modify: `crates/hermes-tools-builtin/src/lib.rs`
- Test: `crates/hermes-tools-builtin/tests/test_todo.rs`

### 步骤 1: 写测试

```rust
// crates/hermes-tools-builtin/tests/test_todo.rs
use hermes_tools_builtin::todo_tools::{TodoItem, TodoStore, TodoParams};
use serde_json::json;

#[test]
fn test_todo_store_write_replace() {
    let store = TodoStore::new();
    let items = vec![
        TodoItem { id: "1".into(), content: "Task 1".into(), status: "pending".into() },
        TodoItem { id: "2".into(), content: "Task 2".into(), status: "in_progress".into() },
    ];
    let result = store.write(items, false);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, "1");
}

#[test]
fn test_todo_store_write_merge() {
    let store = TodoStore::new();
    let initial = vec![TodoItem { id: "1".into(), content: "Task 1".into(), status: "pending".into() }];
    store.write(initial, false);

    let update = vec![TodoItem { id: "1".into(), content: "Task 1 updated".into(), status: "completed".into() }];
    let result = store.write(update, true);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].status, "completed");
}

#[test]
fn test_todo_store_read_empty() {
    let store = TodoStore::new();
    let result = store.read();
    assert!(result.is_empty());
}

#[test]
fn test_todo_store_invalid_status_defaults_to_pending() {
    let store = TodoStore::new();
    let items = vec![TodoItem { id: "1".into(), content: "Task".into(), status: "invalid".into() }];
    let result = store.write(items, false);
    assert_eq!(result[0].status, "pending");
}

#[test]
fn test_todo_store_empty_id_defaults_to_question_mark() {
    let store = TodoStore::new();
    let items = vec![TodoItem { id: "".into(), content: "Task".into(), status: "pending".into() }];
    let result = store.write(items, false);
    assert_eq!(result[0].id, "?");
}

#[test]
fn test_todo_store_dedupe_by_id() {
    let store = TodoStore::new();
    let items = vec![
        TodoItem { id: "1".into(), content: "First".into(), status: "pending".into() },
        TodoItem { id: "1".into(), content: "Second".into(), status: "completed".into() },
    ];
    let result = store.write(items, false);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "Second"); // last occurrence wins
}

#[test]
fn test_todo_store_summary_counts() {
    let store = TodoStore::new();
    let items = vec![
        TodoItem { id: "1".into(), content: "p".into(), status: "pending".into() },
        TodoItem { id: "2".into(), content: "i".into(), status: "in_progress".into() },
        TodoItem { id: "3".into(), content: "c".into(), status: "completed".into() },
        TodoItem { id: "4".into(), content: "x".into(), status: "cancelled".into() },
    ];
    store.write(items, false);
    let summary = store.summary();
    assert_eq!(summary.pending, 1);
    assert_eq!(summary.in_progress, 1);
    assert_eq!(summary.completed, 1);
    assert_eq!(summary.cancelled, 1);
}
```

### 步骤 2: 运行测试确认失败

Run: `cargo test -p hermes-tools-builtin test_todo -- --nocapture 2>&1 | head -30`
Expected: FAIL — module not found

### 步骤 3: 创建 `todo_tools.rs`

```rust
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
            let mut existing: std::collections::HashMap<String, usize> = self.items
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
        let mut s = TodoSummary { total: self.items.len(), pending: 0, in_progress: 0, completed: 0, cancelled: 0 };
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
        Self { store: Arc::new(RwLock::new(TodoStore::new())) }
    }
}

impl Default for TodoTool {
    fn default() -> Self { Self::new() }
}

impl Clone for TodoTool {
    fn clone(&self) -> Self {
        Self { store: Arc::clone(&self.store) }
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
    fn name(&self) -> &str { "todo" }

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

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: TodoParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let merge = params.merge.unwrap_or(false);
        let store = self.store.write();
        let items = if let Some(todos) = params.todos {
            store.write(todos, merge)
        } else {
            store.read()
        };

        let summary = store.summary();
        Ok(json!({ "todos": items, "summary": summary }).to_string())
    }
}
```

### 步骤 4: 运行测试确认通过

Run: `cargo test -p hermes-tools-builtin test_todo -- --nocapture`
Expected: PASS

### 步骤 5: 更新 `lib.rs`

```rust
// 在 pub mod terminal_tools; 后添加
pub mod todo_tools;
pub mod clarify_tools;

// 在 pub use file_tools... 后添加
pub use todo_tools::{TodoTool, TodoStore};

// 在 register_builtin_tools 函数中注册
registry.register(TodoTool::new());
```

### 步骤 6: 运行测试确认通过

Run: `cargo test -p hermes-tools-builtin test_todo -- --nocapture`
Expected: PASS

### 步骤 7: 提交

```bash
git add crates/hermes-tools-builtin/src/todo_tools.rs crates/hermes-tools-builtin/src/lib.rs crates/hermes-tools-builtin/tests/test_todo.rs
git commit -m "feat(tools-builtin): add TodoTool for task list management

- TodoStore supports replace and merge modes
- Deduplication by id, last occurrence wins
- Status validation defaults to pending
- Summary counts for each status
- Implements Tool trait with JSON schema

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: ClarifyTool 核心实现

**Files:**
- Create: `crates/hermes-tools-builtin/src/clarify_tools.rs`
- Modify: `crates/hermes-tools-builtin/src/lib.rs`
- Test: `crates/hermes-tools-builtin/tests/test_clarify.rs`

### 步骤 1: 写测试

```rust
// crates/hermes-tools-builtin/tests/test_clarify.rs
use hermes_tools_builtin::clarify_tools::{ClarifyTool, AskUserFn};
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

fn make_ask_user答题_fn() -> AskUserFn {
    Box::new(|question: String, _choices: Option<Vec<String>>| -> String {
        if question.contains("favorite") {
            "B".to_string()
        } else {
            "user answer".to_string()
        }
    })
}

#[test]
fn test_clarify_tool_name() {
    let tool = ClarifyTool::new(make_ask_user答题_fn());
    assert_eq!(tool.name(), "clarify");
}

#[test]
fn test_clarify_tool_parameters_schema() {
    let tool = ClarifyTool::new(make_ask_user答题_fn());
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    let props = params.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("question"));
    assert!(props.contains_key("choices"));
}

#[test]
fn test_clarify_tool_execute_with_question() {
    let ask_user = make_ask_user答题_fn();
    let tool = ClarifyTool::new(ask_user);
    let args = json!({ "question": "What is your favorite color?" });
    let result = tool.execute_sync(args).unwrap();
    assert!(result.contains("What is your favorite color"));
    assert!(result.contains("user answer"));
}

#[test]
fn test_clarify_tool_execute_empty_question() {
    let tool = ClarifyTool::new(make_ask_user答题_fn());
    let args = json!({ "question": "" });
    let result = tool.execute_sync(args);
    assert!(result.is_err());
}

#[test]
fn test_clarify_tool_execute_with_choices() {
    let ask_user = Box::new(|_q: String, choices: Option<Vec<String>>| -> String {
        choices.map(|c| c.join(",")).unwrap_or_default()
    });
    let tool = ClarifyTool::new(ask_user);
    let args = json!({ "question": "Pick one", "choices": ["A", "B", "C"] });
    let result = tool.execute_sync(args).unwrap();
    assert!(result.contains("A, B, C"));
}

#[test]
fn test_clarify_tool_execute_no_callback() {
    let tool = ClarifyTool::new_noop();
    let args = json!({ "question": "Hello?" });
    let result = tool.execute_sync(args).unwrap();
    assert!(result.contains("not available"));
}
```

### 步骤 2: 运行测试确认失败

Run: `cargo test -p hermes-tools-builtin test_clarify -- --nocapture 2>&1 | head -30`
Expected: FAIL — module not found

### 步骤 3: 创建 `clarify_tools.rs`

```rust
//! clarify_tools — 用户交互工具
//!
//! 支持多选一和开放式问题，回调函数由平台层注入。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// 最大选项数
const MAX_CHOICES: usize = 4;

/// 用户交互回调类型
pub type AskUserFn = Box<dyn Fn(String, Option<Vec<String>>) -> String + Send + Sync>;

/// ClarifyTool — 用户交互工具
///
/// 通过回调函数向用户提问，支持多选一和开放式问题。
pub struct ClarifyTool {
    ask_user: Arc<AskUserFn>,
}

impl ClarifyTool {
    /// 使用提供的回调创建
    pub fn new(ask_user: AskUserFn) -> Self {
        Self { ask_user: Arc::new(ask_user) }
    }

    /// 创建无回调版本（返回友好错误）
    pub fn new_noop() -> Self {
        Self::new(Box::new(|_, _| String::new()))
    }

    /// 同步执行（供测试用）
    pub fn execute_sync(&self, args: serde_json::Value) -> Result<String, ToolError> {
        #[derive(Debug, Deserialize)]
        struct ClarifyParams {
            question: Option<String>,
            #[serde(default)]
            choices: Option<Vec<String>>,
        }

        let params: ClarifyParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let question = params.question.unwrap_or_default();
        if question.trim().is_empty() {
            return Err(ToolError::InvalidArgs("question text is required".into()));
        }

        let mut choices = params.choices;
        if let Some(ref c) = choices {
            if c.len() > MAX_CHOICES {
                choices = Some(c[..MAX_CHOICES].to_vec());
            }
        }

        // 检查回调是否为空（noop）
        let is_noop = self.ask_user.as_ref()
            .call(&(), (&question, &choices))
            .is_empty();

        if is_noop {
            return Ok(json!({
                "error": "Clarify tool is not available in this execution context."
            }).to_string());
        }

        let user_response = (self.ask_user)(question.clone(), choices.clone());
        Ok(json!({
            "question": question,
            "choices_offered": choices,
            "user_response": user_response
        }).to_string())
    }
}

impl Clone for ClarifyTool {
    fn clone(&self) -> Self {
        Self { ask_user: Arc::clone(&self.ask_user) }
    }
}

impl std::fmt::Debug for ClarifyTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClarifyTool").finish()
    }
}

#[async_trait]
impl Tool for ClarifyTool {
    fn name(&self) -> &str { "clarify" }

    fn description(&self) -> &str {
        "Ask the user a question when you need clarification or feedback."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to present to the user."
                },
                "choices": {
                    "type": "array",
                    "items": { "type": "string" },
                    "maxItems": 4,
                    "description": "Up to 4 answer choices. Omit for open-ended question."
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        self.execute_sync(args)
    }
}
```

### 步骤 4: 运行测试确认通过

Run: `cargo test -p hermes-tools-builtin test_clarify -- --nocapture`
Expected: PASS

### 步骤 5: 更新 `lib.rs`

```rust
// 在 pub use todo_tools::TodoTool 后添加
pub use clarify_tools::ClarifyTool;

// 在 register_builtin_tools 函数中注册
// 注意：ClarifyTool 需要回调，平台层会单独创建和注册
```

### 步骤 6: 运行测试确认通过

Run: `cargo test -p hermes-tools-builtin test_clarify -- --nocapture`
Expected: PASS

### 步骤 7: 提交

```bash
git add crates/hermes-tools-builtin/src/clarify_tools.rs crates/hermes-tools-builtin/src/lib.rs crates/hermes-tools-builtin/tests/test_clarify.rs
git commit -m "feat(tools-builtin): add ClarifyTool for user interaction

- Supports multiple choice (up to 4 options) and open-ended questions
- Callback injected by platform layer (CLI/Gateway)
- Noop version returns friendly error when callback unavailable
- Validates question is non-empty

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: 集成验证

**Files:**
- Modify: `crates/hermes-tools-builtin/src/lib.rs`

### 步骤 1: 确认 TodoTool 注册

检查 `register_builtin_tools` 是否注册了 `TodoTool::new()`。

### 步骤 2: 运行完整编译

Run: `cargo check --all 2>&1 | tail -20`
Expected: 编译通过，无错误

### 步骤 3: 运行完整测试

Run: `cargo test -p hermes-tools-builtin 2>&1 | tail -20`
Expected: 所有测试通过

### 步骤 4: 提交

```bash
git add -A
git commit -m "chore: integrate TodoTool and ClarifyTool

- TodoTool registered in register_builtin_tools
- ClarifyTool available for platform-layer injection
- All tests passing

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## 验收清单

### TodoTool
- [ ] `merge=false` 替换整个列表
- [ ] `merge=true` 按 id 合并更新
- [ ] 无参数调用读取当前列表
- [ ] 状态验证正确（无效转为 pending）
- [ ] 空 id 设为 "?"
- [ ] 返回完整列表和统计
- [ ] 按 id 去重，保留最后出现

### ClarifyTool
- [ ] question 参数必填
- [ ] choices 最多 4 个
- [ ] 空 question 返回 `ToolError::InvalidArgs`
- [ ] 回调正常调用
- [ ] 无回调时返回友好错误 JSON

### 集成
- [ ] `cargo check --all` 通过
- [ ] `cargo test -p hermes-tools-builtin` 通过
- [ ] `TodoTool` 在 `register_builtin_tools` 中注册

---

## 关键类型对照

| 类型/方法 | 定义位置 |
|-----------|----------|
| `Tool` trait | `hermes-tool-registry/src/lib.rs` |
| `ToolContext`, `ToolError` | `hermes-core/src/lib.rs` |
| `TodoStore` | `crates/hermes-tools-builtin/src/todo_tools.rs` |
| `ClarifyTool::new(ask_user)` | `crates/hermes-tools-builtin/src/clarify_tools.rs` |
| `register_builtin_tools()` | `crates/hermes-tools-builtin/src/lib.rs` |
