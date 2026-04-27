# Hermes Agent 用户指南

Hermes Agent 是一个 AI 助手 CLI 工具，支持多 Provider、流式输出、工具执行、会话管理和技能系统。

---

## 目录

1. [快速开始](#快速开始)
2. [安装和配置](#安装和配置)
3. [聊天交互](#聊天交互)
4. [会话管理](#会话管理)
5. [工具系统](#工具系统)
6. [技能系统](#技能系统)
7. [模型管理](#模型管理)
8. [配置管理](#配置管理)
9. [网关服务](#网关服务)
10. [多 Provider 支持](#多-provider-支持)

---

## 快速开始

### 基本用法

```bash
# 启动交互式聊天（使用默认模型 openai/gpt-4o）
hermes chat

# 使用指定模型
hermes chat --model anthropic/claude-3-5-sonnet

# 继续之前的会话
hermes chat --session <session-id>

# 禁用工具执行（纯聊天模式）
hermes chat --no-tools
```

### 构建项目

```bash
# 构建所有 crate
cargo build --all

# 运行 hermes CLI
cargo run -- chat

# 运行测试
cargo test --all
```

---

## 安装和配置

### 环境变量

Hermes Agent 通过环境变量或配置文件获取 API 密钥：

| 环境变量 | 说明 |
|----------|------|
| `OPENAI_API_KEY` | OpenAI API 密钥 |
| `ANTHROPIC_API_KEY` | Anthropic API 密钥 |
| `OPENROUTER_API_KEY` | OpenRouter API 密钥 |
| `GLM_API_KEY` | 智谱 AI API 密钥 |
| `MINIMAX_API_KEY` | MiniMax API 密钥 |
| `KIMI_API_KEY` | Kimi API 密钥 |
| `DEEPSEEK_API_KEY` | DeepSeek API 密钥 |
| `QWEN_API_KEY` | 阿里云百炼 API 密钥 |

### 配置文件

配置文件位于 `~/.config/hermes-agent/config.toml`：

```toml
[providers]
default = "openai/gpt-4o"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
base_url = "https://api.anthropic.com"
```

---

## 聊天交互

### 启动聊天

```bash
hermes chat
hermes chat -m openai/gpt-4o
hermes chat --model anthropic/claude-3-5-sonnet-20241022
```

### REPL 命令

在聊天过程中，可以使用以下命令：

| 命令 | 说明 |
|------|------|
| `/help` | 显示帮助信息 |
| `/model [name]` | 显示或切换模型 |
| `/tools` | 列出可用工具 |
| `/context` | 显示对话上下文信息 |
| `/reset` | 清空对话历史 |
| `/compact` | 压缩对话上下文 |
| `/version` | 显示版本信息 |

### 多凭据模式

使用 `--credentials` 参数启用多凭据轮询：

```bash
hermes chat --credentials "openai:sk-key1,openai:sk-key2"
```

---

## 会话管理

### 列出所有会话

```bash
hermes session list
```

### 查看会话详情

```bash
hermes session show <session-id>
```

### 搜索会话

```bash
hermes session search "关键词"
```

### 删除会话

```bash
hermes session delete <session-id>
```

---

## 工具系统

Hermes Agent 内置多种工具，可以在聊天中调用。

### 列出可用工具

```bash
hermes tools list
```

### 启用/禁用工具

```bash
hermes tools enable <tool-name>
hermes tools disable <tool-name>
```

### 内置工具

| 工具 | 说明 |
|------|------|
| `ReadFile` | 读取文件内容 |
| `WriteFile` | 写入文件内容 |
| `EditFile` | 编辑文件（精确修改） |
| `Grep` | 在文件中搜索内容 |
| `Terminal` | 执行终端命令 |
| `Glob` | 查找匹配的文件 |
| `Browser` | 网页浏览工具 |
| `Todo` | 待办事项管理 |
| `Clarify` | 获取用户确认 |

### 工具执行流程

1. Agent 分析用户请求
2. 确定需要调用的工具
3. 执行工具并获取结果
4. 将结果反馈给模型
5. 生成最终回复

---

## 技能系统

技能是可扩展的功能模块，可以从市场安装或从 git 仓库安装。

### 列出已安装技能

```bash
hermes skills list
```

### 搜索技能市场

```bash
hermes skills search <关键词>
```

### 安装技能

```bash
# 从市场安装
hermes skills install <category/name>

# 从 git 仓库安装
hermes skills install --git <git-url> --category <category> --name <name> --branch <branch>
```

### 卸载技能

```bash
hermes skills uninstall <category/name>
```

### Git 安装示例

```bash
hermes skills install \
  --git https://github.com/user/skill-repo \
  --category custom \
  --name my-skill \
  --branch main
```

仓库中的 `.md` 文件会被扫描为技能，每个文件支持 frontmatter 元数据：

```yaml
---
name: my-skill
description: 这是一个技能描述
category: custom
version: 1.0.0
---
```

---

## 模型管理

### 列出可用模型

```bash
hermes model list
```

### 设置默认模型

```bash
hermes model set <provider/model>
```

### 查看模型信息

```bash
hermes model info <provider/model>
```

### 支持的模型

| Provider | 模型示例 |
|---------|----------|
| `openai` | `gpt-4o`, `gpt-4-turbo`, `gpt-3.5-turbo` |
| `anthropic` | `claude-3-5-sonnet-20241022`, `claude-4-opus-20250514` |
| `openrouter` | `openai/gpt-4o`, `anthropic/claude-3.5-sonnet` |
| `glm` | `glm-4`, `glm-4-flash` |
| `minimax` | `MiniMax-Text-01` |
| `kimi` | `moonshot-v1-8k` |
| `deepseek` | `deepseek-chat` |
| `qwen` | `qwen-turbo`, `qwen-plus` |

---

## 配置管理

### 显示配置

```bash
hermes config show
```

### 读取配置项

```bash
hermes config get <key>
```

### 设置配置项

```bash
hermes config set <key> <value>
```

### 编辑配置文件

```bash
hermes config edit
```

---

## 网关服务

网关服务提供 HTTP API 接口，可以接收外部请求并转发给 Agent 处理。

### 启动网关

```bash
hermes gateway start --port 8080
```

### 查看状态

```bash
hermes gateway status
```

### 停止网关

```bash
hermes gateway stop
```

### 初始化配置

```bash
hermes gateway setup
```

---

## 多 Provider 支持

Hermes Agent 支持同时使用多个 LLM Provider，根据模型 ID 自动选择。

### 模型 ID 格式

```
provider/model-name
```

例如：
- `openai/gpt-4o`
- `anthropic/claude-3-5-sonnet-20241022`
- `deepseek/deepseek-chat`

### Provider 路由

| 模型前缀 | Provider |
|---------|----------|
| `openai/` | OpenAI |
| `anthropic/` | Anthropic |
| `openrouter/` | OpenRouter |
| `glm/` | 智谱 AI |
| `minimax/` | MiniMax |
| `kimi/` | Kimi (Moonshot) |
| `deepseek/` | DeepSeek |
| `qwen/` | 阿里云百炼 |

### 流式输出

所有 Provider 支持流式输出，回复会实时显示在终端：

```bash
hermes chat --model openai/gpt-4o
# 回复会逐字显示
```

---

## 上下文压缩

当对话历史过长时，Hermes Agent 会自动压缩上下文：

- **Hybrid 模式**：结合 LLM 摘要和确定性提取
- **元数据提取**：提取文件路径、符号引用、决策记录
- **摘要生成**：保留对话的核心信息

压缩会自动触发，无需手动操作。

---

## 常见问题

### Q: API 密钥在哪里获取？

- **OpenAI**: https://platform.openai.com/api-keys
- **Anthropic**: https://console.anthropic.com/
- **其他 Provider**: 访问各自开发者平台

### Q: 如何查看调试日志？

```bash
RUST_LOG=debug hermes chat
```

### Q: 如何报告问题？

请访问项目 GitHub 仓库提交 Issue。

---

## 命令速查表

| 命令 | 说明 |
|------|------|
| `hermes chat` | 启动聊天 |
| `hermes chat -m <model>` | 使用指定模型聊天 |
| `hermes chat -s <id>` | 继续指定会话 |
| `hermes session list` | 列出会话 |
| `hermes session show <id>` | 查看会话 |
| `hermes session search <q>` | 搜索会话 |
| `hermes session delete <id>` | 删除会话 |
| `hermes model list` | 列出模型 |
| `hermes model set <m>` | 设置默认模型 |
| `hermes tools list` | 列出工具 |
| `hermes tools enable <t>` | 启用工具 |
| `hermes tools disable <t>` | 禁用工具 |
| `hermes skills list` | 列出技能 |
| `hermes skills install <s>` | 安装技能 |
| `hermes skills search <q>` | 搜索技能 |
| `hermes config show` | 显示配置 |
| `hermes config edit` | 编辑配置 |
| `hermes gateway start` | 启动网关 |
| `hermes gateway status` | 网关状态 |
