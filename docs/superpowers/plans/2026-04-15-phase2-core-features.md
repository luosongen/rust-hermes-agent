# Phase 2: Core Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现 Memory Manager 和 Context Compressor，完善内存管理和上下文压缩

**Architecture:**
- MemoryManager 协调多个 MemoryProvider，类似于 Python 版本的设计
- ContextCompressor 使用 LLM 生成摘要，压缩中间对话轮次
- 现有的 SqliteSessionStore 已支持 FTS5，无需额外实现

**Tech Stack:** Rust, sqlx, async_trait, hermes_core, hermes_memory

---

## 实现顺序

1. **MemoryManager** - 内存管理器，协调 providers
2. **ContextCompressor** - 上下文压缩器，LLM 摘要生成
3. **集成测试** - 验证完整流程

---

## Task 1: MemoryManager

**Files:**
- Create: `crates/hermes-memory/src/memory_manager.rs`
- Modify: `crates/hermes-memory/src/lib.rs`

### MemoryProvider Trait

首先定义 MemoryProvider trait（类似于 Python 版本）:

```rust
use async_trait::async_trait;

/// MemoryProvider trait - 内存提供者接口
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// 提供者名称
    fn name(&self) -> &str;

    /// 获取工具 schema（用于动态工具注册）
    fn get_tool_schemas(&self) -> Vec<serde_json::Value>;

    /// 构建系统提示词块
    fn system_prompt_block(&self) -> String;

    /// 预取相关记忆
    fn prefetch(&self, query: &str, session_id: &str) -> String;

    /// 队列预取（异步）
    fn queue_prefetch(&self, query: &str, session_id: &str);

    /// 同步一轮对话
    fn sync_turn(&self, user_content: &str, assistant_content: &str, session_id: &str);

    /// 处理工具调用
    fn handle_tool_call(&self, tool_name: &str, args: serde_json::Value) -> Result<String, String>;
}
```

### MemoryManager 结构

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// MemoryManager - 协调多个内存提供者
///
/// 内置提供者始终优先注册。只允许一个外部（非内置）提供者。
pub struct MemoryManager {
    providers: Vec<Arc<dyn MemoryProvider>>,
    tool_to_provider: HashMap<String, Arc<dyn MemoryProvider>>,
    has_external: bool,
}

impl MemoryManager {
    pub fn new() -> Self;

    /// 注册内存提供者
    pub fn add_provider(&mut self, provider: Arc<dyn MemoryProvider>) -> Result<(), String>;

    /// 获取所有注册提供者
    pub fn providers(&self) -> Vec<Arc<dyn MemoryProvider>>;

    /// 构建系统提示词
    pub fn build_system_prompt(&self) -> String;

    /// 预取所有相关记忆
    pub async fn prefetch_all(&self, query: &str, session_id: &str) -> String;

    /// 同步一轮对话到所有提供者
    pub async fn sync_all(&self, user_content: &str, assistant_content: &str, session_id: &str);

    /// 获取所有工具 schema
    pub fn get_all_tool_schemas(&self) -> Vec<serde_json::Value>;

    /// 处理工具调用
    pub async fn handle_tool_call(&self, tool_name: &str, args: serde_json::Value) -> Result<String, String>;
}
```

---

## Task 2: ContextCompressor

**Files:**
- Create: `crates/hermes-memory/src/context_compressor.rs`
- Modify: `crates/hermes-memory/src/lib.rs`

### ContextCompressor 结构

```rust
use hermes_core::{ChatRequest, LlmProvider};
use std::sync::Arc;

/// 上下文压缩器 - 使用 LLM 摘要压缩长对话
///
/// Algorithm:
/// 1. 保护头部消息（系统提示词 + 第一轮对话）
/// 2. 保护尾部消息（最近 N 轮，按 token 预算）
/// 3. 对中间轮次生成结构化摘要
/// 4. 后续压缩时迭代更新摘要
pub struct ContextCompressor {
    /// 当前使用的 LLM Provider
    llm: Arc<dyn LlmProvider>,

    /// 模型名称
    model: String,

    /// 上下文窗口大小
    context_length: usize,

    /// 压缩阈值（token 数量百分比）
    threshold_percent: f32,

    /// 摘要目标比率
    summary_target_ratio: f32,

    /// 保护的第一条消息数量
    protect_first_n: usize,

    /// 保护的最新消息数量
    protect_last_n: usize,

    /// 尾部 token 预算
    tail_token_budget: usize,

    /// 最大摘要 token 数
    max_summary_tokens: usize,

    /// 压缩计数
    compression_count: usize,

    /// 上一次摘要（用于迭代更新）
    previous_summary: Option<String>,

    /// 摘要失败冷却时间
    summary_failure_cooldown_until: f64,
}

impl ContextCompressor {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        model: String,
        context_length: usize,
    ) -> Self;

    /// 检查是否需要压缩
    pub fn should_compress(&self, prompt_tokens: usize) -> bool;

    /// 压缩对话消息
    pub async fn compress(
        &self,
        messages: Vec<hermes_core::Message>,
        current_tokens: Option<usize>,
        focus_topic: Option<&str>,
    ) -> Result<Vec<hermes_core::Message>, String>;

    /// 生成摘要
    async fn generate_summary(
        &self,
        turns_to_summarize: &[hermes_core::Message],
        focus_topic: Option<&str>,
    ) -> Result<String, String>;

    /// 剪枝旧工具结果
    fn prune_old_tool_results(
        &self,
        messages: &[hermes_core::Message],
        protect_tail_tokens: usize,
    ) -> Vec<hermes_core::Message>;

    /// 清理孤立的工具调用/结果对
    fn sanitize_tool_pairs(&self, messages: Vec<hermes_core::Message>) -> Vec<hermes_core::Message>;
}
```

### 摘要提示词模板

```rust
const SUMMARY_PREFIX: &str = "[CONTEXT COMPACTION — REFERENCE ONLY]";

const SUMMARY_TEMPLATE: &str = r#"## Goal
[What the user is trying to accomplish]

## Progress
### Done
[Completed work]

## Remaining Work
[What remains to be done]"#;
```

---

## Task 3: 集成

**Files:**
- Modify: `crates/hermes-memory/src/lib.rs`
- Modify: `crates/hermes-core/src/agent.rs` (如果需要)

导出模块:

```rust
pub mod memory_manager;
pub mod context_compressor;

pub use memory_manager::{MemoryManager, MemoryProvider};
pub use context_compressor::ContextCompressor;
```

---

## 验收清单

- [ ] MemoryManager 编译通过
- [ ] MemoryProvider trait 定义完整
- [ ] ContextCompressor 编译通过
- [ ] 单元测试通过
- [ ] `cargo check --all` 通过

---

## 关键文件

| 文件 | 职责 |
|------|------|
| `crates/hermes-memory/src/memory_manager.rs` | MemoryManager 实现 (新增) |
| `crates/hermes-memory/src/context_compressor.rs` | ContextCompressor 实现 (新增) |
| `crates/hermes-memory/src/lib.rs` | 导出新模块 |
| `crates/hermes-memory/src/sqlite_store.rs` | 已有 FTS5 支持 |
| `crates/hermes-memory/src/session.rs` | Session/Mession 类型定义 |

---

## 下一步

Phase 2 完成后，进入 Phase 3: Tools + MCP
