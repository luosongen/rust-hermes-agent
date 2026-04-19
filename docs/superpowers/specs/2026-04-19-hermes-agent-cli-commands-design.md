# Hermes Agent CLI Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `crate-hermes-agent` 二进制程序的所有 CLI 命令（Model、Session、Tools、Skills、Gateway），使其成为功能完整的命令行工具。

**Architecture:** 采用方案A——每个命令独立初始化其依赖。命令处理器根据需要创建自己的 Provider、SessionStore、ToolRegistry 等组件，而非依赖共享的全局状态。这确保了命令之间的解耦和可测试性。

**Tech Stack:** Rust (tokio async runtime), clap CLI parsing, hermes-core, hermes-memory, hermes-tool-registry, hermes-skills

---

## 1. File Structure

```
crates/hermes-cli/src/
├── commands/
│   ├── mod.rs              # 模块声明和重导出
│   ├── model.rs            # model list/info/set 命令
│   ├── session.rs          # session list/show/search/delete 命令
│   ├── tools.rs            # tools list/enable/disable 命令
│   ├── skills.rs           # skills list/install/uninstall/search 命令
│   └── gateway.rs          # gateway start/stop/status/setup 命令
├── commands.rs             # Cli/Commands enum 定义（已存在）
└── lib.rs                  # 模块导出（已存在）
```

---

## 2. Command Handlers

### 2.1 Model Commands (`model.rs`)

**依赖:**
- `hermes_core::config::Config` 读取模型配置

**命令:**
- `model list` — 列出所有可用模型（从配置和 provider 支持列表）
- `model info <model-id>` — 显示模型详细信息
- `model set <model-id>` — 设置默认模型

### 2.2 Session Commands (`session.rs`)

**依赖:**
- `hermes_memory::{SessionStore, SqliteSessionStore}`
- `hermes_memory::NewSession`

**命令:**
- `session list` — 列出所有会话（来自 SQLite）
- `session show <session-id>` — 显示会话详情和消息数
- `session search <query>` — 搜索会话内容
- `session delete <session-id>` — 删除会话

**初始化模式:**
```rust
let session_store = SqliteSessionStore::new("hermes.db".into()).await?;
```

### 2.3 Tools Commands (`tools.rs`)

**依赖:**
- `hermes_tool_registry::ToolRegistry`
- `hermes_tools_builtin::register_builtin_tools`

**命令:**
- `tools list` — 列出所有已注册工具
- `tools enable <tool-name>` — 启用工具（从配置启用）
- `tools disable <tool-name>` — 禁用工具（从配置禁用）

**初始化模式:**
```rust
let tool_registry = ToolRegistry::new();
register_builtin_tools(&tool_registry);
```

### 2.4 Skills Commands (`skills.rs`)

**依赖:**
- `hermes_skills::SkillManager`
- `hermes_skills::SkillRegistry`

**命令:**
- `skills list` — 列出所有已安装 skills
- `skills install <skill-source>` — 安装 skill（从目录或 git）
- `skills uninstall <skill-name>` — 卸载 skill
- `skills search <query>` — 搜索可用 skills

**初始化模式:**
```rust
let skill_manager = SkillManager::new(skills_dir);
let skill_registry = skill_manager.load_registry()?;
```

### 2.5 Gateway Commands (`gateway.rs`)

**依赖:**
- `hermes_gateway::{Gateway, GatewayConfig}`
- `hermes_platform_telegram::TelegramAdapter` 或 `hermes_platform_wecom::WeComAdapter`

**命令:**
- `gateway start --platform <telegram|wecom>` — 启动网关
- `gateway stop` — 停止网关
- `gateway status` — 显示网关状态
- `gateway setup --platform <telegram|wecom>` — 交互式配置网关

**初始化模式:**
```rust
let gateway = Gateway::new(gateway_config);
gateway.start().await?;
```

---

## 3. Error Handling

- 使用 `anyhow::Result<()>` 便于错误传播
- 每命令打印友好错误信息到 stderr
- 关键错误记录到日志

---

## 4. Implementation Order

1. `commands/mod.rs` — 创建模块结构
2. `commands/model.rs` — 最简单，无外部依赖
3. `commands/session.rs` — 需要 async SQLite
4. `commands/tools.rs` — 工具注册表
5. `commands/skills.rs` — Skill 管理器
6. `commands/gateway.rs` — HTTP 服务器
7. 更新 `commands.rs` 导入新模块
8. 更新 `lib.rs` 导出

---

## 5. Success Criteria

- `cargo build --all` 编译通过
- 所有命令有 `--help` 输出
- `model list` 返回可用模型
- `session list` 返回会话列表
- `tools list` 返回工具列表
- `skills list` 返回 skills 列表
- `gateway status` 返回网关状态
