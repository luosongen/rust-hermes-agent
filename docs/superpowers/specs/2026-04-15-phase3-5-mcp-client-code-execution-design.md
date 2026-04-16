# Phase 3.5: MCP Client + Code Execution Design Specification

> **Status:** Approved
> **Date:** 2026-04-15

## 概述

Phase 3.5 在 `hermes-tools-extended` 中新增两个工具：

1. **McpClientBridge** — MCP Client，实现 Universal MCP Gateway
2. **CliExecutor** — 工具式代码执行器

---

## 模块结构

```
crates/hermes-tools-extended/src/
├── mcp_client.rs       # MCP Client Bridge（新）
├── cli_executor.rs     # CLI 代码执行工具（新）
├── web_search.rs       # 已有
├── web_fetch.rs        # 已有
├── cron_scheduler.rs   # 已有
├── mcp_server.rs       # 已有
└── lib.rs              # 更新导出
```

---

## 模块 1: McpClientBridge

### 目标

将 hermes-agent 作为 **MCP Client**，通过 stdio 连接到外部 MCP 服务器，动态加载远程工具到本地注册表。

### 工作流程

```
1. 启动      → McpClientBridge::new(name, command, args) 启动子进程
2. 初始化    → 发送 initialize 请求，获取 server capabilities
3. 发现工具  → 发送 tools/list 请求，获取远程工具列表
4. 工具注册  → 将远程工具包装为本地 McpTool 实例
5. 调用执行  → tools/call 请求转发到远程 server，结果返回
```

### 接口设计

```rust
/// MCP Client 工具
pub struct McpClientBridge {
    server_name: String,
    command: String,
    args: Vec<String>,
    child: Option<ChildProcess>,
    writer: Arc<Mutex<BufWriter<ChildStdin>>>,
    reader: Arc<Mutex<BufReader<ChildStdout>>>,
}

impl McpClientBridge {
    /// 创建并连接到一个 MCP 服务器
    pub async fn connect(
        server_name: &str,
        command: &str,
        args: &[String],
    ) -> Result<Self, ToolError>;

    /// 断开连接
    pub fn disconnect(&mut self) -> Result<(), ToolError>;

    /// 列出已连接服务器的工具
    pub fn list_tools(&self) -> Vec<ToolDefinition>;

    /// 调用远程工具
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, ToolError>;
}
```

### 工具参数 (JSON Schema)

```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": ["connect", "disconnect", "list", "call"],
      "description": "操作类型"
    },
    "server_name": {
      "type": "string",
      "description": "MCP 服务器名称（用于 namespacing）"
    },
    "command": {
      "type": "string",
      "description": "启动命令（如 npx, python, ./mcp-server）"
    },
    "args": {
      "type": "array",
      "items": { "type": "string" },
      "description": "命令参数"
    },
    "tool_name": {
      "type": "string",
      "description": "要调用的工具名称（action=call 时）"
    },
    "arguments": {
      "type": "object",
      "description": "工具参数（action=call 时）"
    },
    "timeout_ms": {
      "type": "integer",
      "default": 30000,
      "description": "超时时间（毫秒）"
    }
  },
  "required": ["action"]
}
```

### 工具名称 Namespacing

外部 MCP 工具以 `{server_name}.{tool}` 格式注册到本地注册表。

示例：
- `github.create_issue`
- `filesystem.read_file`
- `slack.post_message`

### MCP 协议实现

**协议版本:** `2024-11-05`

**支持的 JSON-RPC 方法:**

| 方法 | 方向 | 描述 |
|------|------|------|
| `initialize` | Client→Server | 初始化连接 |
| `tools/list` | Client→Server | 列出可用工具 |
| `tools/call` | Client→Server | 调用工具 |
| `notifications/initialized` | Client→Server | 初始化完成通知 |

### JSON-RPC 请求/响应格式

```json
// tools/list 请求
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {}
}

// tools/list 响应
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "create_issue",
        "description": "Create a GitHub issue",
        "inputSchema": {
          "type": "object",
          "properties": {
            "title": { "type": "string" },
            "body": { "type": "string" }
          }
        }
      }
    ]
  }
}

// tools/call 请求
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "create_issue",
    "arguments": { "title": "Bug", "body": "Details" }
  }
}

// tools/call 响应
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [
      { "type": "text", "text": "Issue created: #123" }
    ]
  }
}
```

### 错误处理

| 错误类型 | MCP 错误码 | 处理方式 |
|----------|------------|----------|
| 连接失败 | -32000 | 返回 `ToolError::Execution` |
| 解析错误 | -32700 | 记录日志，重试 |
| 方法不存在 | -32601 | 返回 `ToolError::InvalidParameters` |
| 无效参数 | -32602 | 返回 `ToolError::InvalidParameters` |
| 请求超时 | -32001 | 杀死进程，返回错误 |

---

## 模块 2: CliExecutor

### 目标

提供安全的**工具式代码执行**能力，通过外部 CLI 解释器（python, node, bash）执行代码脚本，支持流式输出和资源限制。

### 支持的解释器

| 解释器 | 命令 | 用途 |
|--------|------|------|
| `python` | `python3` | Python 脚本执行 |
| `node` | `node` | JavaScript/TypeScript |
| `bash` | `bash` | Shell 命令 |

### 接口设计

```rust
/// CLI 执行器工具
pub struct CliExecutor {
    allowed_interpreters: HashMap<String, InterpreterConfig>,
    default_timeout_ms: u64,
}

pub struct InterpreterConfig {
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,        // 固定参数（如 ["-c"] for bash）
    pub max_timeout_ms: u64,
    pub max_buffer_kb: usize,
}

impl CliExecutor {
    pub fn new(config: ExecutorConfig) -> Self;

    pub fn execute(
        &self,
        interpreter: &str,
        script: &str,
        args: Vec<String>,
        timeout_ms: Option<u64>,
    ) -> Result<ExecutionResult, ToolError>;

    pub fn execute_streaming(
        &self,
        interpreter: &str,
        script: &str,
        args: Vec<String>,
        timeout_ms: Option<u64>,
    ) -> Result<Receiver<String>, ToolError>;
}

pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}
```

### 工具参数 (JSON Schema)

```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": ["execute", "list"],
      "description": "操作类型"
    },
    "interpreter": {
      "type": "string",
      "enum": ["python", "node", "bash"],
      "description": "解释器类型"
    },
    "script": {
      "type": "string",
      "description": "要执行的脚本内容"
    },
    "args": {
      "type": "array",
      "items": { "type": "string" },
      "description": "额外命令行参数"
    },
    "timeout_ms": {
      "type": "integer",
      "default": 30000,
      "description": "超时时间（毫秒）"
    },
    "stream": {
      "type": "boolean",
      "default": true,
      "description": "是否流式输出"
    }
  },
  "required": ["action", "interpreter", "script"]
}
```

### 执行流程

```
1. 验证      → 检查解释器是否在允许列表且已启用
2. 权限检查  → 检查 ToolContext 中的执行权限
3. 构建命令  → 拼接解释器命令和脚本
4. 启动进程  → 使用 tokio::process::Command 启动
5. 流式输出  → stdout/stderr 通过 mpsc channel 流式返回
6. 等待完成  → 进程结束后收集退出码
7. 返回结果  → ExecutionResult { stdout, stderr, exit_code, duration_ms }
```

### 资源限制

| 限制项 | 默认值 | 说明 |
|--------|--------|------|
| `timeout_ms` | 30000 | 最大运行时间 |
| `max_buffer_kb` | 1024 | 输出缓冲区大小 |

### 配置结构 (ExecutorConfig)

```rust
#[derive(Clone)]
pub struct ExecutorConfig {
    pub python: InterpreterConfig,
    pub node: InterpreterConfig,
    pub bash: InterpreterConfig,
}

#[derive(Clone)]
pub struct InterpreterConfig {
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub max_timeout_ms: u64,
    pub max_buffer_kb: usize,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            python: InterpreterConfig {
                enabled: true,
                command: "python3".to_string(),
                args: vec!["-".to_string()],  // stdin mode
                max_timeout_ms: 60000,
                max_buffer_kb: 2048,
            },
            node: InterpreterConfig {
                enabled: true,
                command: "node".to_string(),
                args: vec!["-e".to_string()],
                max_timeout_ms: 60000,
                max_buffer_kb: 2048,
            },
            bash: InterpreterConfig {
                enabled: true,
                command: "bash".to_string(),
                args: vec!["-c".to_string()],
                max_timeout_ms: 30000,
                max_buffer_kb: 1024,
            },
        }
    }
}
```

### 错误处理

| 错误类型 | 原因 | 返回 |
|----------|------|------|
| `InterpreterDisabled` | 解释器未启用 | `ToolError::Execution("Interpreter not allowed")` |
| `Timeout` | 进程超时 | 杀死进程，返回部分输出 |
| `BufferOverflow` | 输出超过限制 | 返回截断输出 + 警告 |
| `ExecutionFailed` | 进程崩溃 | stderr 内容作为错误 |

---

## ToolContext 扩展

```rust
// hermes-core/src/tool.rs

pub struct ToolContext {
    pub session_id: String,
    pub user_id: Option<String>,
    // 新增字段
    pub config: Arc<Config>,
    pub mcp_clients: Arc<RwLock<HashMap<String, McpClientBridge>>>,
    pub executor_config: Arc<ExecutorConfig>,
}
```

---

## 与现有架构的集成

```
Agent (hermes-core)
    ↓
ToolDispatcher → ToolRegistry
                       ↓
              ┌────────┴────────┐
              ↓                 ↓
      BuiltinTools      ExtendedTools
                                ↓
                    ┌───────────────┐
                    ↓       ↓      ↓
              WebSearch  McpClient  CliExecutor
                          Bridge
```

---

## 实现顺序

1. **Task 1:** `McpClientBridge` — MCP Client 核心实现
2. **Task 2:** `McpClientBridge` — 工具注册和 namespacing
3. **Task 3:** `CliExecutor` — CLI 执行器核心实现
4. **Task 4:** `CliExecutor` — 流式输出支持
5. **Task 5:** 集成测试和最终验证

---

## 验收清单

- [ ] `McpClientBridge::connect` 成功连接到外部 MCP 服务器
- [ ] 远程工具正确注册为 `{server_name}.{tool}` 格式
- [ ] `tools/call` 正确转发请求到远程 server
- [ ] `CliExecutor` 正确执行 python/node/bash 脚本
- [ ] 流式输出正常工作
- [ ] 超时和资源限制生效
- [ ] `cargo check --all` 通过
- [ ] `cargo test -p hermes-tools-extended` 通过

---

## 关键依赖

| Crate | 用途 |
|-------|------|
| `tokio::process` | 子进程管理 |
| `tokio::io::AsyncBufReadExt` | stdout/stderr 流式读取 |
| `serde_json` | JSON-RPC 协议解析 |
| `futures` | Stream 支持 |

---

## 下一步 (Phase 4)

- Phase 4: Skills System — LLM 原生 skill 调用
