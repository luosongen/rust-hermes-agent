# Phase X: VisionTool + MemoryTool 设计规格

> **Status:** Draft
> **Date:** 2026-04-16
> **Goal:** 实现 VisionTool（图像分析）和 MemoryTool（跨会话记忆）

---

## 概述

本阶段为 hermes-agent Rust 版添加两个新工具：

1. **VisionTool** — 支持图像/截图分析，调用云视觉模型 API
2. **MemoryTool** — 跨会话持久化记忆（K-V 存储，search 后续升级为向量搜索）

---

## 模块结构

```
crates/hermes-tools-extended/src/
├── vision.rs    # VisionTool 实现
└── memory.rs    # MemoryTool 实现
```

注册入口：`crates/hermes-tools-extended/src/lib.rs` + `register_extended_tools()`

---

## 模块 1: VisionTool

### 目标

让 Agent 能够分析图像内容（截图、照片、图表等），通过云服务视觉模型（GPT-4V / Claude Vision / Gemini）返回分析结果。

### 接口设计

```rust
// 参数
struct VisionParams {
    image: String,        // 图片 URL 或本地路径
    prompt: String,       // 分析指令
    model: Option<String>, // 可选，默认使用配置的模型
}

// 工具名: "vision"
```

### 参数 JSON Schema

```json
{
  "type": "object",
  "properties": {
    "image": {
      "type": "string",
      "description": "图片 URL 或本地路径"
    },
    "prompt": {
      "type": "string",
      "description": "分析指令，默认 '描述这张图片的内容'"
    },
    "model": {
      "type": "string",
      "description": "可选，视觉模型名称"
    }
  },
  "required": ["image"]
}
```

### 实现逻辑

1. 解析 `image` — 如果是本地路径，读取并转为 base64；如果是 URL，直接使用
2. 获取 LLM Provider（通过 `hermes-core` 的 `LlmProvider` trait）
3. 构造多模态请求（text + image），调用 provider 的 `chat()` 方法
4. 返回文本分析结果

### 多模态请求构造

Provider 需要支持 `Content::Image` 类型。当前 `hermes-provider` 的 `ChatRequest.messages` 为 `Vec<Message>`，每个 `Message.content` 为 `Vec<Content>`。

```rust
// Message.content 支持多种 Content 类型
enum Content {
    Text(String),
    Image { url: String, detail: String },  // 新增
}
```

### 错误处理

| 场景 | 处理 |
|------|------|
| 图片读取失败 | `ToolError::Execution("Failed to read image file")` |
| Provider 不支持视觉 | `ToolError::Execution("Provider does not support vision")` |
| 网络超时 | `ToolError::Timeout` |
| 模型返回空 | `ToolError::Execution("Empty response from vision model")` |

---

## 模块 2: MemoryTool

### 目标

提供跨会话的持久化记忆存储。Agent 可以主动写入信息（`memory_set`）并在后续会话中检索（`memory_get` / `memory_search`）。

当前阶段实现 K-V 存储。向量搜索（基于 embedding 的语义搜索）作为后续功能。

### 接口设计

```rust
// 三种操作：set / get / search
struct MemoryParams {
    action: String,  // "set" | "get" | "search"
    key: Option<String>,
    value: Option<String>,
    query: Option<String>,  // search 专用
    limit: Option<usize>,   // search 返回条数限制，默认 5
}
```

### 参数 JSON Schema

```json
{
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
}
```

### 存储设计

复用 `hermes-memory` 的 SQLite SessionStore，新增 `memory` 表：

```sql
CREATE TABLE IF NOT EXISTS memory (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### 实现逻辑

- **memory_set**: `INSERT OR REPLACE INTO memory (key, value, created_at, updated_at)`，无则创建，有则更新
- **memory_get**: `SELECT value FROM memory WHERE key = ?`，无结果返回 null
- **memory_search**: `SELECT key, value FROM memory WHERE value LIKE '%query%' LIMIT ?`（当前阶段用子串匹配，后续升级为向量搜索）

### 错误处理

| 场景 | 处理 |
|------|------|
| key 不存在 (get) | 返回 `null` |
| 数据库错误 | `ToolError::Execution("Memory error: ...")` |
| 参数缺失 | `ToolError::InvalidArgs` |

---

## 依赖关系

```
hermes-tools-extended
    ├── hermes-core (LlmProvider, Message, Content, ToolContext, ToolError)
    ├── hermes-memory (SessionStore, SqliteSessionStore)
    └── hermes-tool-registry (Tool trait, ToolRegistry)

vision tool:
    └── hermes-provider (需支持多模态 Content::Image)

memory tool:
    └── hermes-memory 的 SqliteSessionStore 实例
```

---

## 验收清单

### VisionTool
- [ ] 支持本地文件路径输入（base64 编码）
- [ ] 支持 URL 输入
- [ ] 通过 provider 的 `chat()` 发送多模态请求
- [ ] 错误处理正确（文件读取失败、provider 不支持 vision 等）
- [ ] 单元测试通过

### MemoryTool
- [ ] `memory_set` 正确持久化（创建 + 更新）
- [ ] `memory_get` 正确读取，不存在时返回 null
- [ ] `memory_search` 子串匹配正确
- [ ] 数据库 schema 初始化正确
- [ ] 单元测试通过

### 集成
- [ ] `cargo check --all` 通过
- [ ] `cargo test -p hermes-tools-extended` 通过

---

## 实现顺序

1. **VisionTool** 核心实现（vision.rs）
2. **MemoryTool** 核心实现（memory.rs）
3. **hermes-provider 多模态支持**（如需要，检查并扩展 Content 枚举）
4. **注册工具**（lib.rs 更新）
5. **单元测试**
6. **集成测试**

---

## 关键文件

| 文件 | 职责 |
|------|------|
| `crates/hermes-tools-extended/src/vision.rs` | VisionTool 实现 |
| `crates/hermes-tools-extended/src/memory.rs` | MemoryTool 实现 |
| `crates/hermes-tools-extended/src/lib.rs` | 模块导出 + 注册 |
| `crates/hermes-core/src/types.rs` | Content 枚举（可能需要扩展 Image 变体）|
