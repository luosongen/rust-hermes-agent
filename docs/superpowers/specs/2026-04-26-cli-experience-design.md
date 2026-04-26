# CLI 体验改进设计

> **Goal:** 整合现有 UI 组件到 REPL，增强加载动画和命令补全，提供更流畅的交互体验

> **Architecture:** 保持 tokio 异步 I/O 框架，集成异步 readline 替代品，连接已定义但未使用的 UI 组件

> **Tech Stack:** tokio async I/O, tokio-rl (或类似异步 readline 库), ANSI escape codes

---

## 1. 概述

### 1.1 当前状态

`hermes-cli` 的 REPL 使用原始的 `tokio::io::stdin`，没有：
- 行编辑（backspace, delete, arrow keys）
- 命令历史导航
- Tab 自动补全
- 加载动画

### 1.2 目标

整合并增强已有 UI 组件，提供：
1. 异步 readline 功能（行编辑 + 历史 + 补全）
2. 加载动画（Agent 处理期间）
3. 命令参数补全

---

## 2. 架构设计

### 2.1 组件关系

```
┌─────────────────────────────────────────────────────────┐
│                     REPL 主循环                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │ LineReader  │→ │   Agent     │→ │ Streaming   │   │
│  │ (异步readline)│ │  (async)   │  │  Output     │   │
│  └─────────────┘  └─────────────┘  └─────────────┘   │
│         ↑                ↑                ↑           │
│         │                │                │           │
│  ┌──────┴──────┐ ┌──────┴──────┐ ┌──────┴──────┐   │
│  │CommandHistory│ │Completer   │ │LoadingAnim │   │
│  │(已定义/未连接)│ │(增强补全)  │ │(实现动画)   │   │
│  └─────────────┘ └─────────────┘ └─────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### 2.2 约束

- **保持异步** — 继续使用 tokio I/O，不引入同步阻塞
- **异步 readline** — 使用 `tokio-rl` 或类似方案
- **不破坏现有功能** — Agent 核心逻辑保持不变

---

## 3. 组件设计

### 3.1 LineReader（新增/整合）

**职责:** 异步读取用户输入，提供行编辑和历史功能

**接口:**
```rust
pub struct LineReader {
    history: CommandHistory,
    completer: SlashCommandCompleter,
}

impl LineReader {
    pub async fn read_line(&mut self, prompt: &str) -> Result<String>;
}
```

**功能:**
- 行编辑（backspace, delete, Ctrl-W）
- 命令历史（up/down arrows）
- Tab 补全触发
- 异步非阻塞

**整合点:**
- 替换 `chat.rs` 中的 `BufReader::new(stdin).lines()`
- 连接 `CommandHistory` 提供历史功能

### 3.2 LoadingAnimation（实现）

**职责:** 在 Agent 处理期间显示加载动画

**接口:**
```rust
pub struct LoadingAnimation {
    enabled: Arc<AtomicBool>,
    message: String,
}

impl LoadingAnimation {
    pub fn new() -> Self;
    pub fn start(&self, message: &str);
    pub fn stop(&self);
}
```

**动画字符序列:**
`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` (旋转)

**实现细节:**
- 使用 ANSI escape codes: `\r` 回车 + `\x1B[K` 清除行
- 在独立 task 中运行动画循环
- `stop()` 设置 `enabled = false`，动画停止

**整合点:**
- `chat.rs` REPL 循环中，Agent 调用前 `start()`，返回后 `stop()`

### 3.3 CommandCompleter（增强）

**职责:** 提供命令参数补全

**当前状态:**
```rust
pub fn complete_args(&self, _command: &str, _partial: &str) -> Vec<String> {
    // TODO: 根据命令类型提供参数补全
    Vec::new()
}
```

**需要实现的补全:**

| 命令 | 参数补全 |
|------|---------|
| `/model` | 可用模型列表 (从配置读取) |
| `/context` | `compress`, `clear`, `status` |
| `/tokens` | `status` |
| `/system` | `prompt`, `role` |

**整合点:**
- 连接到 LineReader 的 Tab 事件

### 3.4 StreamingOutput（整合）

**职责:** 显示 Agent 实时响应

**当前状态:** `StreamingOutput` 已定义但未连接到 REPL

**需要做:**
- 在 `chat.rs` 中实例化 `StreamingOutput`
- Agent `run_conversation` 支持流式输出回调
- 回调调用 `StreamingOutput::write()`

---

## 4. 文件变更

| 文件 | 变更 |
|------|------|
| `crates/hermes-cli/src/chat.rs` | 整合所有 UI 组件，替换原始 stdin |
| `crates/hermes-cli/src/ui/line_reader.rs` | 新增：异步 readline 封装 |
| `crates/hermes-cli/src/ui/streaming_output.rs` | 实现 `start_loading()`/`stop_loading()` |
| `crates/hermes-cli/src/ui/completer.rs` | 实现 `complete_args()` |
| `crates/hermes-cli/src/ui/mod.rs` | 导出新模块 |
| `crates/hermes-cli/Cargo.toml` | 添加 `tokio-rl` 依赖 |

---

## 5. 依赖添加

```toml
# Cargo.toml (hermes-cli)
tokio-rl = "0.4"  # 或类似异步 readline 库
```

---

## 6. 成功标准

1. REPL 支持行编辑（backspace, delete, arrow keys）
2. 命令历史可以通过 up/down 箭头访问
3. Tab 触发命令补全
4. Agent 处理期间显示加载动画
5. `/model` 命令提供模型列表补全
6. 所有现有测试继续通过

---

## 7. 风险与缓解

| 风险 | 缓解 |
|------|------|
| tokio-rl 库不稳定 | 如果库不满足需求，回退到基础实现 |
| 动画闪烁 | 测试不同 escape code 组合 |
| 性能影响 | 动画在独立 task 中运行，不阻塞主循环 |
