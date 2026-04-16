# Phase X: Agent 核心增强设计规格

> **Status:** Draft
> **Date:** 2026-04-16
> **Goal:** 实现 DelegateTool（并行子代理）和 ContextCompressor（上下文压缩重写）

---

## 概述

本阶段增强 hermes-agent Rust 版的两个核心能力：

1. **DelegateTool** — 支持并行子代理调度的委托机制
2. **ContextCompressor** — 智能上下文压缩引擎（重写）

---

## 模块结构

```
crates/hermes-core/src/
├── delegate/
│   ├── mod.rs           # 导出
│   ├── delegate_tool.rs # 核心实现
│   └── types.rs         # 参数和结果类型
├── context_engine/
│   ├── mod.rs           # 导出
│   └── compressor.rs    # ContextCompressor 实现
└── traits/
    └── context_engine.rs # ContextEngine trait

crates/hermes-tools-builtin/src/
└── lib.rs               # 注册 DelegateTool
```

---

## 模块 1: DelegateTool

### 目标

让 Agent 能够将任务委托给并行执行的子代理（subagent），每个子代理拥有独立的对话上下文和受限的工具集，父 Agent 阻塞等待所有子代理完成后汇总结果。

### 工作流程

```
1. 接收参数 → goal, context, toolsets, max_iterations
2. 权限检查 → 确保未超出最大深度 (MAX_DEPTH = 2)
3. Credential 解析 → 子代理继承父 Agent 的 provider credentials
4. 工具集交集 → 父子工具集交集，剥离 blocked 工具
5. 创建子代理 → AIAgent(quiet_mode, ephemeral_system_prompt)
6. 并行执行 → ThreadPool 或 tokio::task::spawn_blocking
7. 收集结果 → 汇总所有子代理的 summary/tool_trace
8. 返回 → DelegateResult JSON
```

### 接口设计

#### DelegateTool 结构体

```rust
pub struct DelegateTool {
    agent: Arc<Mutex<AIAgent>>,
    max_concurrent: usize,
    max_depth: u8,
}

impl DelegateTool {
    pub fn new(agent: Arc<Mutex<AIAgent>>) -> Self;
    pub fn with_config(agent: Arc<Mutex<AIAgent>>, max_concurrent: usize, max_depth: u8) -> Self;
}
```

#### 参数类型

```rust
/// 单任务委托参数
#[derive(Debug, Deserialize)]
pub struct DelegateParams {
    pub goal: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub toolsets: Option<Vec<String>>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}

/// 批量委托参数
#[derive(Debug, Deserialize)]
pub struct BatchDelegateParams {
    pub tasks: Vec<DelegateTask>,
    #[serde(default)]
    pub max_concurrent: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DelegateTask {
    pub goal: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub toolsets: Option<Vec<String>>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
}
```

#### 结果类型

```rust
#[derive(Debug, Serialize)]
pub struct DelegateResult {
    pub status: DelegateStatus,
    pub summary: String,
    pub api_calls: u32,
    pub duration_ms: u64,
    pub model: String,
    pub exit_reason: String,
    #[serde(default)]
    pub tool_trace: Vec<ToolTraceEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DelegateStatus {
    Completed,
    Failed,
    Interrupted,
    Error,
}

#[derive(Debug, Serialize)]
pub struct ToolTraceEntry {
    pub tool: String,
    pub args_bytes: usize,
    pub result_bytes: usize,
    pub status: String,
}
```

### Tool trait 实现

```rust
#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str { "delegate" }

    fn description(&self) -> &str {
        "Delegate a task to subagent(s) that run in parallel with restricted toolsets"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "goal": { "type": "string", "description": "Single task goal" },
                        "context": { "type": "string", "description": "Background context" },
                        "toolsets": { "type": "array", "items": { "type": "string" } },
                        "max_iterations": { "type": "integer", "default": 50 }
                    },
                    "required": ["goal"]
                },
                {
                    "properties": {
                        "tasks": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "goal": { "type": "string" },
                                    "context": { "type": "string" },
                                    "toolsets": { "type": "array", "items": { "type": "string" } },
                                    "max_iterations": { "type": "integer" }
                                },
                                "required": ["goal"]
                            }
                        },
                        "max_concurrent": { "type": "integer", "default": 3 }
                    },
                    "required": ["tasks"]
                }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError>;
}
```

### 关键设计决策

1. **tokio::spawn 而非进程** — 子代理在 tokio task 中运行，共享父进程 credential pool
2. **credential 继承** — 子代理通过 `Arc<CredentialPool>` 引用共享父 Agent 的凭证
3. **深度限制 MAX_DEPTH = 2** — 父=0，子=1，递归委托只允许到第 2 层
4. **blocked 工具** — `delegate_task`, `clarify`, `memory`, `send_message`, `execute_code` 强制剥离
5. **quiet_mode** — 子代理设置 `quiet_mode=true`，减少冗余输出
6. **心跳线程** — 子代理运行时每 30s 触发 `parent._touch_activity()` 防止 gateway 超时

### 错误处理

| 场景 | 处理 |
|------|------|
| 深度超限 | 返回 `ToolError::Execution("Max delegation depth exceeded")` |
| 并发超限 | 返回 `ToolError::Execution("Too many concurrent tasks")` |
| 子代理异常 | 记录为 `status: "error"`，异常消息放入 `summary` |
| 无可用工具 | 子代理返回 `status: "failed"` + 说明 |

---

## 模块 2: ContextCompressor

### 目标

在对话接近模型 token 上限时，自动压缩历史消息。保留头部（系统提示 + 首批 exchange）和尾部（最近 N token），用 LLM 总结中间部分。

### ContextEngine Trait

```rust
#[async_trait]
pub trait ContextEngine: Send + Sync {
    fn name(&self) -> &str;
    fn should_compress(&self, prompt_tokens: usize) -> bool;
    async fn compress(
        &self,
        messages: &[Message],
        prompt_tokens: usize,
        focus_topic: Option<&str>,
    ) -> Result<Vec<Message>, ToolError>;
    fn on_session_reset(&mut self);
    fn get_status(&self) -> CompressionStatus;
}

#[derive(Debug, Clone, Serialize)]
pub struct CompressionStatus {
    pub compression_count: usize,
    pub current_tokens: usize,
    pub threshold_tokens: usize,
    pub model: String,
}
```

### ContextCompressor 实现

```rust
pub struct ContextCompressor {
    model: String,
    context_length: usize,
    threshold_percent: f32,
    tail_token_ratio: f32,
    summary_ratio: f32,
    min_summary_tokens: usize,
    max_summary_tokens: usize,
    auxiliary_provider: Arc<dyn LlmProvider>,
    previous_summary: Mutex<Option<String>>,
    compression_count: AtomicUsize,
}

impl ContextCompressor {
    pub fn new(
        model: String,
        context_length: usize,
        auxiliary_provider: Arc<dyn LlmProvider>,
    ) -> Self;

    pub fn with_thresholds(
        self,
        threshold_percent: f32,
        tail_token_ratio: f32,
        summary_ratio: f32,
    ) -> Self;
}
```

### 4-Phase 压缩 Pipeline

```
compress(messages, focus_topic)
    │
    ├─[1] prune_tool_results()
    │       遍历消息，将超过 200 字符的旧 tool result 替换为占位符
    │
    ├─[2] find_tail_boundary()
    │       从尾部向前计算 token 预算，确定保留的尾部起始位置
    │
    ├─[3] summarize_middle()
    │       将中间部分发送给 LLM，返回结构化摘要
    │       包含: Goal, Constraints, Progress, Key Decisions,
    │             Pending User Asks, Relevant Files, Remaining Work
    │
    └─[4] assemble_and_sanitize()
            拼接: head + summary + tail
            处理 orphaned tool-call/tool-result 对
```

### 摘要 Prompt 模板

```
You are a summarization agent creating a context checkpoint for a DIFFERENT
assistant that continues the conversation.

Do NOT respond to any questions in this conversation.

## Goal
## Constraints & Preferences
## Progress (Done / In Progress / Blocked)
## Key Decisions
## Resolved Questions
## Pending User Asks
## Relevant Files
## Remaining Work
## Critical Context
## Tools & Patterns
```

### Token 预算计算

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `threshold_percent` | 0.50 | 触发压缩的 token 上限比例 |
| `tail_token_ratio` | 0.20 | tail 占总预算的比例 |
| `summary_ratio` | 0.20 | 摘要 LLM 输出 token 比例 |
| `min_summary_tokens` | 2000 | 摘要最小 token 数 |
| `max_summary_tokens` | min(context_length * 0.05, 12000) | 摘要最大 token 数 |

### Orphan 处理

```rust
/// 确保 tool-call 和 tool-result 配对完整
fn sanitize_tool_pairs(messages: &mut Vec<Message>) {
    // 1. 收集所有存活的 tool_call_id
    // 2. 移除没有匹配 result 的 tool_result
    // 3. 为没有 result 的 tool_call 插入 stub result
}
```

### 失败处理

- LLM summarization 失败时，插入静态 fallback 摘要消息
- 设置 10 分钟 cooldown 防止连续失败
- 记录 `compression_count` 和错误日志

### 与现有 ContextCompressor 的区别

| 方面 | 现有实现 | 本次重写 |
|------|----------|----------|
| 摘要方式 | 直接截断 | LLM 语义总结 |
| Tail 处理 | 固定条数 | 基于 token 预算 |
| Middle 处理 | 直接丢弃 | 语义压缩保留关键信息 |
| Re-compactions | 无 | 传递 `_previous_summary` 迭代更新 |
| Orphan 处理 | 无 | 完整 sanitize_tool_pairs |
| Focus topic | 无 | 支持 `/compress <topic>` |

---

## 依赖关系

```
hermes-core
    │
    ├── hermes-provider (LlmProvider trait)
    ├── hermes-memory (SessionStore)
    └── hermes-tool-registry (Tool trait)
```

---

## 验收清单

### DelegateTool
- [ ] 单任务委托执行成功
- [ ] 批量并行委托（最多 3 并发）成功
- [ ] 深度限制 MAX_DEPTH=2 生效
- [ ] Blocked 工具被正确剥离
- [ ] Credential pool 正确继承
- [ ] 超时和异常被正确捕获
- [ ] 单元测试通过

### ContextCompressor
- [ ] 触发阈值正确计算
- [ ] Tail token 预算正确
- [ ] 摘要 LLM 调用成功
- [ ] Re-compactions 正确传递前次摘要
- [ ] Orphan tool pairs 正确处理
- [ ] 失败时 fallback 摘要生效
- [ ] 单元测试通过

### 集成
- [ ] `cargo check --all` 通过
- [ ] `cargo test -p hermes-core` 通过

---

## 实现顺序

1. **Task 1:** DelegateTool 核心类型定义 (`delegate/types.rs`)
2. **Task 2:** DelegateTool 单任务执行
3. **Task 3:** DelegateTool 批量并行执行
4. **Task 4:** DelegateTool 测试
5. **Task 5:** ContextEngine trait + ContextCompressor 结构体
6. **Task 6:** 4-phase 压缩 pipeline
7. **Task 7:** ContextCompressor 测试
8. **Task 8:** 集成测试和最终验证

---

## 关键文件

| 文件 | 职责 |
|------|------|
| `crates/hermes-core/src/delegate/mod.rs` | 模块导出 |
| `crates/hermes-core/src/delegate/types.rs` | DelegateParams, DelegateResult 等类型 |
| `crates/hermes-core/src/delegate/delegate_tool.rs` | DelegateTool 核心实现 |
| `crates/hermes-core/src/traits/context_engine.rs` | ContextEngine trait |
| `crates/hermes-core/src/context_engine/mod.rs` | 模块导出 |
| `crates/hermes-core/src/context_engine/compressor.rs` | ContextCompressor 实现 |
| `crates/hermes-core/src/lib.rs` | 扩展导出 |
