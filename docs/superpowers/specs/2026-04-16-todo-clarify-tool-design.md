# TodoTool + ClarifyTool Design Spec

> **Status:** Draft
> **Date:** 2026-04-16
> **Goal:** 实现 TodoTool（任务管理）和 ClarifyTool（用户交互）两个内置工具

---

## 概述

本阶段为 hermes-agent Rust 版添加两个新的内置工具：

1. **TodoTool** — 会话内任务列表管理（分解复杂任务、跟踪进度）
2. **ClarifyTool** — 向用户提问，支持多选一或开放式问题

---

## 模块结构

```
crates/hermes-tools-builtin/src/
├── lib.rs              # 模块导出 + register_builtin_tools 更新
├── todo_tools.rs       # TodoTool 实现
└── clarify_tools.rs    # ClarifyTool 实现
```

---

## 模块 1: TodoTool

### 目标

让 Agent 能够在长会话中管理任务列表，在上下文压缩后保持任务状态。

### 接口设计

```rust
// 工具名: "todo"
struct TodoParams {
    todos: Option<Vec<TodoItem>>,  // 写入时提供
    merge: Option<bool>,            // false=替换, true=合并
}

struct TodoItem {
    id: String,
    content: String,
    status: String,  // "pending" | "in_progress" | "completed" | "cancelled"
}
```

### 参数 JSON Schema

```json
{
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
}
```

### 错误处理

| 场景 | 处理 |
|------|------|
| 读取空列表 | 返回空数组 |
| 无效状态 | 默认设为 "pending" |
| 空 id | 设为 "?" |

---

## 模块 2: ClarifyTool

### 目标

让 Agent 能够向用户提问，获取用户的选择或自由文本回复。

### 接口设计

```rust
// 工具名: "clarify"
struct ClarifyParams {
    question: String,
    choices: Option<Vec<String>>,  // 最多 4 个选项
}
```

### 参数 JSON Schema

```json
{
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
}
```

### 实现逻辑

ClarifyTool 需要一个回调函数来处理用户交互。由于 Tool trait 的 execute 方法是同步的，回调通过结构体字段注入：

```rust
pub struct ClarifyTool {
    ask_user: Arc<dyn Fn(String, Option<Vec<String>>) -> String + Send + Sync>,
}
```

平台层（CLI / Gateway）在创建 ClarifyTool 时注入具体的回调实现。

### 错误处理

| 场景 | 处理 |
|------|------|
| question 为空 | 返回 `ToolError::InvalidArgs` |
| choices > 4 | 截断到 4 个 |
| 无回调 | 返回错误 JSON（而非 panic）|

---

## 验收清单

### TodoTool
- [ ] `merge=false` 替换整个列表
- [ ] `merge=true` 按 id 合并更新
- [ ] 无参数调用读取当前列表
- [ ] 状态验证正确（无效转为 pending）
- [ ] 返回完整列表和统计

### ClarifyTool
- [ ] question 参数必填
- [ ] choices 最多 4 个
- [ ] 空 question 返回错误
- [ ] 回调正常调用
- [ ] 无回调时返回友好错误

### 集成
- [ ] `cargo check --all` 通过
- [ ] `cargo test -p hermes-tools-builtin` 通过
- [ ] `register_builtin_tools` 注册新工具

---

## 关键文件

| 文件 | 职责 |
|------|------|
| `crates/hermes-tools-builtin/src/todo_tools.rs` | TodoStore + TodoTool |
| `crates/hermes-tools-builtin/src/clarify_tools.rs` | ClarifyTool |
| `crates/hermes-tools-builtin/src/lib.rs` | 模块导出 + 注册更新 |
