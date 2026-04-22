# Hermes Agent 功能实现文档

## 1. ACP (Agent Copilot Protocol) 实现

### 1.1 功能概述

ACP 实现允许 Hermes Agent 在编辑器和其他 ACP 兼容的客户端中作为智能代理使用。主要功能包括：

- **初始化**：响应客户端的初始化请求，返回协议版本和代理信息
- **会话管理**：创建和管理 ACP 会话
- **命令处理**：支持各种 slash 命令，如 `/help`、`/model`、`/tools` 等
- **对话处理**：处理用户输入，执行工具，并返回响应

### 1.2 实现细节

ACP 实现位于 `hermes-acp` crate 中，主要文件：

- [lib.rs](file:///Users/Rowe/ai-projects/rust-hermes-agent/crates/hermes-acp/src/lib.rs)：核心实现

#### 1.2.1 核心结构体

- **`AcpServer`**：ACP 服务器实现，负责处理 ACP 协议请求
- **`SessionManager`**：会话管理器，存储和管理所有活跃会话
- **`SessionState`**：会话状态，存储单个会话的信息和历史记录

#### 1.2.2 主要方法

- **`initialize`**：处理初始化请求，返回代理信息和能力
- **`new_session`**：创建新会话，返回会话 ID
- **`prompt`**：处理用户输入，执行命令或运行对话
- **`handle_slash_command`**：处理 slash 命令
- **命令处理方法**：`cmd_help`、`cmd_model`、`cmd_tools`、`cmd_context`、`cmd_reset`、`cmd_compact`、`cmd_version`

### 1.3 使用方法

#### 1.3.1 初始化 ACP 服务器

```rust
use hermes_acp::AcpServer;
use hermes_core::AgentConfig;

// 创建 ACP 服务器
let agent_config = AgentConfig::default();
let acp_server = AcpServer::new(agent_config);

// 处理初始化请求
let init_response = acp_server.initialize().await;
```

#### 1.3.2 创建新会话

```rust
// 创建新会话，指定工作目录
let session_response = acp_server.new_session("/path/to/workdir").await;
let session_id = session_response.session_id;
```

#### 1.3.3 处理用户输入

```rust
use hermes_acp::ContentBlock;

// 创建内容块
let content_blocks = vec![ContentBlock::Text(TextContentBlock {
    text: "Hello, Hermes!"
})];

// 处理用户输入
let response = acp_server.prompt(content_blocks, session_id).await;
```

#### 1.3.4 使用 Slash 命令

```rust
// 使用 help 命令
let content_blocks = vec![ContentBlock::Text(TextContentBlock {
    text: "/help"
})];

let response = acp_server.prompt(content_blocks, session_id).await;
```

## 2. 定时任务系统

### 2.1 功能概述

定时任务系统允许用户安排工具在指定时间执行，主要功能包括：

- **任务调度**：支持 cron 表达式的任务调度
- **任务管理**：添加、取消、列出任务
- **任务执行**：自动执行计划的工具任务
- **集成**：与工具系统无缝集成

### 2.2 实现细节

定时任务系统位于 `hermes-tools-extended` crate 中，主要文件：

- [cron_scheduler.rs](file:///Users/Rowe/ai-projects/rust-hermes-agent/crates/hermes-tools-extended/src/cron_scheduler.rs)：核心实现

#### 2.2.1 核心结构体

- **`CronScheduler`**：定时任务调度器，负责管理和执行任务
- **`ScheduledJob`**：定时任务结构，包含任务 ID、cron 表达式、工具名称和参数

#### 2.2.2 主要方法

- **`new`**：创建新的任务调度器
- **`set_tool_registry`**：设置工具注册表，用于执行任务
- **`start`**：启动任务执行循环
- **`stop`**：停止任务执行循环
- **`schedule`**：添加新任务
- **`cancel`**：取消任务
- **`list`**：列出所有任务
- **`execute`**：实现 Tool trait，处理工具调用

### 2.3 使用方法

#### 2.3.1 注册 CronScheduler 工具

```rust
use hermes_tool_registry::ToolRegistry;
use hermes_tools_extended::{register_extended_tools, CronScheduler};
use hermes_core::LlmProvider;
use hermes_memory::SqliteSessionStore;
use std::sync::Arc;

// 创建工具注册表
let registry = Arc::new(ToolRegistry::new());

// 创建 LLM 提供商和会话存储
let llm_provider = // 创建 LLM 提供商
let session_store = // 创建会话存储

// 注册扩展工具，包括 CronScheduler
register_extended_tools(registry, llm_provider, session_store);
```

#### 2.3.2 使用 CronScheduler 工具

```rust
use serde_json::json;
use hermes_core::ToolContext;

// 获取 CronScheduler 工具
let cron_scheduler = registry.get("cron_schedule").unwrap();

// 调度任务：每天 9 点执行 web_search 工具
let args = json!({
    "action": "schedule",
    "cron_expression": "0 9 * * *",
    "tool_name": "web_search",
    "tool_args": {"query": "Rust programming news"}
});

let context = ToolContext {
    session_id: "test_session",
    working_directory: std::env::current_dir().unwrap(),
    user_id: None,
    task_id: None,
};

let result = cron_scheduler.execute(args, context).await;
```

#### 2.3.3 管理任务

```rust
// 列出所有任务
let args = json!({
    "action": "list"
});
let result = cron_scheduler.execute(args, context).await;

// 取消任务
let args = json!({
    "action": "cancel",
    "job_id": "job_1"
});
let result = cron_scheduler.execute(args, context).await;
```

### 2.4 Cron 表达式格式

CronScheduler 支持标准的 cron 表达式格式：

- **5 字段格式**：`分 时 日 月 周`
- **6 字段格式**：`秒 分 时 日 月 周`

**示例**：
- `0 9 * * *` - 每天 9 点执行
- `0 0 1 * *` - 每月 1 号执行
- `0 12 * * 1` - 每周一 12 点执行
- `0 */2 * * *` - 每 2 小时执行

## 3. 技术架构

### 3.1 ACP 架构

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ ACP Client  │────│ AcpServer  │────│ Hermes Agent│
└─────────────┘    └─────────────┘    └─────────────┘
        │                  │                  │
        │                  │                  │
        ▼                  ▼                  ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ 会话管理    │    │ 命令处理    │    │ 工具执行    │
└─────────────┘    └─────────────┘    └─────────────┘
```

### 3.2 定时任务架构

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ 任务调度    │────│ 任务执行    │────│ 工具系统    │
└─────────────┘    └─────────────┘    └─────────────┘
        │                  │                  │
        │                  │                  │
        ▼                  ▼                  ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ Cron 解析   │    │ 异步执行    │    │ 工具注册表  │
└─────────────┘    └─────────────┘    └─────────────┘
```

## 4. 注意事项

### 4.1 ACP 实现

- ACP 服务器需要与 Hermes Agent 核心功能集成
- 会话状态存储在内存中，重启后会丢失
- 目前不支持持久化会话

### 4.2 定时任务系统

- 任务执行依赖于工具注册表中的工具
- 任务执行是异步的，不会阻塞主线程
- 任务执行失败会记录错误日志，但不会影响其他任务
- 重启后任务会丢失，需要重新调度

## 5. 未来扩展

### 5.1 ACP 扩展

- 添加会话持久化
- 支持更多 ACP 协议特性
- 实现认证和权限管理

### 5.2 定时任务扩展

- 添加任务持久化
- 支持更复杂的任务依赖关系
- 实现任务执行历史和统计
- 添加任务执行结果通知

## 6. 总结

通过实现 ACP 支持和定时任务系统，Hermes Agent 现在具备了更完整的功能，与 Python 版本更加接近。这些功能使 Hermes Agent 在编辑器和其他环境中更加实用，同时通过定时任务系统可以实现自动化操作，提高用户体验。