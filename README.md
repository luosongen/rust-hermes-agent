# Hermes Agent

[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

🤖 **多提供商 AI Agent 框架** — 支持 OpenAI、Anthropic 等多种 LLM 后端，内置工具执行、技能系统和消息平台适配器。

## ✨ 特性

- 🔌 **多 LLM 提供商支持** — OpenAI、Anthropic Claude、自定义端点
- 🛠️ **内置工具集** — 文件操作、终端执行、网页抓取、代码执行等
- 🧠 **技能系统** — 可扩展的程序化记忆，支持安装/创建自定义技能
- 💬 **多平台适配** — Telegram、企业微信、飞书、钉钉、微信、邮件、SMS
- 📦 **会话持久化** — SQLite 存储对话历史，支持断点续聊
- 🔄 **智能重试** — 指数退避、凭证池轮换、限流追踪
- 🎯 **上下文压缩** — 自动管理长对话的 token 预算

## 🚀 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/NousResearch/rust-hermes-agent.git
cd rust-hermes-agent

# 构建
cargo build --release

# 二进制文件位于 target/release/hermes
```

### 基本使用

```bash
# 设置 API 密钥
export OPENAI_API_KEY="sk-xxx"

# 启动交互式聊天
hermes chat

# 指定模型
hermes chat -m anthropic/claude-3-5-sonnet-20241022

# 继续已有会话
hermes chat -s <session-id>
```

## 📖 命令概览

| 命令 | 说明 |
|------|------|
| `hermes chat` | 交互式 AI 对话 |
| `hermes model` | 模型管理（列出/设置/查看详情） |
| `hermes session` | 会话管理（列出/搜索/删除） |
| `hermes tools` | 工具管理（列出/启用/禁用） |
| `hermes skills` | 技能管理（列出/搜索/安装） |
| `hermes config` | 配置管理（读取/写入/编辑） |
| `hermes gateway` | 网关服务（消息平台 Webhook） |

## 🏗️ 架构

```
┌─────────────────────────────────────────────────────────────┐
│                     hermes-cli (CLI 入口)                    │
├─────────────────────────────────────────────────────────────┤
│                     hermes-core (Agent 核心)                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ Agent 循环   │  │ Tool 调度   │  │ Context Compressor  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
├─────────────────────────────────────────────────────────────┤
│  hermes-provider  │  hermes-tool-registry  │  hermes-memory │
├─────────────────────────────────────────────────────────────┤
│               hermes-tools-builtin (内置工具集)               │
│    ReadFile │ WriteFile │ Terminal │ WebSearch │ Skills    │
├─────────────────────────────────────────────────────────────┤
│               hermes-gateway (消息平台网关)                   │
│   Telegram │ WeCom │ Feishu │ DingTalk │ Weixin │ Email    │
└─────────────────────────────────────────────────────────────┘
```

## 📦 Crate 结构

| Crate | 职责 |
|-------|------|
| `hermes-cli` | 命令行解析与交互式 REPL |
| `hermes-core` | Agent 核心逻辑、trait 定义 |
| `hermes-provider` | LLM Provider 实现 |
| `hermes-tool-registry` | 工具注册与调度 |
| `hermes-tools-builtin` | 内置工具集 |
| `hermes-tools-extended` | 扩展工具集（MCP、视觉等） |
| `hermes-memory` | 会话存储（SQLite） |
| `hermes-skills` | 技能系统 |
| `hermes-gateway` | HTTP 网关服务器 |
| `hermes-platform-*` | 各平台适配器 |

## ⚙️ 配置

配置文件位置：`~/.config/hermes-agent/config.toml`

```toml
[defaults]
model = "openai/gpt-4o"
tools_enabled = true
max_iterations = 90

[provider.openai]
api_key = "sk-xxx"

[provider.anthropic]
api_key = "sk-ant-xxx"

[gateway]
host = "0.0.0.0"
port = 8080
```

### 环境变量

| 变量 | 说明 |
|------|------|
| `HERMES_DEFAULT_MODEL` | 默认模型 |
| `HERMES_OPENAI_API_KEY` | OpenAI API 密钥 |
| `HERMES_ANTHROPIC_API_KEY` | Anthropic API 密钥 |
| `HERMES_TELEGRAM_BOT_TOKEN` | Telegram Bot Token |

**优先级**：CLI 参数 > 环境变量 > 配置文件 > 默认值

## 🔧 开发

```bash
# 运行测试
cargo test --all

# 代码检查
cargo clippy --all

# 格式化
cargo fmt --all

# 构建文档
cargo doc --open
```

## 📄 许可证

MIT License © Nous Research
