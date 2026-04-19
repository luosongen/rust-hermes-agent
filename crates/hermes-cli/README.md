# Hermes CLI

Hermes Agent 命令行工具，提供交互式 AI 对话和多种管理命令。

## 安装

```bash
cargo build -p hermes-cli
```

运行：`cargo run -p hermes-cli -- <命令>`

## 命令

### Chat - 交互式对话

```bash
# 启动聊天，使用默认模型
hermes chat

# 指定模型继续会话
hermes chat --model openai/gpt-4o --session <session-id>

# 禁用工具执行
hermes chat --no-tools

# 使用多凭据（启用负载均衡）
hermes chat --credentials "openai:sk-xxx,anthropic:sk-ant-xxx"
```

### Model - 模型管理

```bash
# 列出所有可用模型
hermes model list

# 查看模型详情
hermes model info --model openai/gpt-4o

# 设置默认模型
hermes model set --model anthropic/claude-3-5-sonnet-20241022
```

### Session - 会话管理

```bash
# 列出所有会话
hermes session list

# 查看会话详情
hermes session show --id <session-id>

# 搜索会话内容
hermes session search --query "如何实现 Rust 异步"

# 删除会话
hermes session delete --id <session-id>
```

### Config - 配置管理

```bash
# 显示完整配置
hermes config show

# 获取单个配置值
hermes config get --key defaults.model

# 设置配置值
hermes config set --key defaults.model --value openai/gpt-4o

# 在编辑器中编辑配置
hermes config edit
```

### Tools - 工具管理

```bash
# 列出所有已注册工具
hermes tools list

# 启用工具
hermes tools enable --tool read_file

# 禁用工具
hermes tools disable --tool terminal
```

### Skills - 技能管理

```bash
# 列出已安装技能
hermes skills list

# 搜索技能
hermes skills search --query "code review"

# 安装技能（待实现）
hermes skills install --skill <source>

# 卸载技能
hermes skills uninstall --skill my-skill
```

### Gateway - 网关服务

```bash
# 查看网关状态
hermes gateway status

# 查看配置说明
hermes gateway setup

# 启动网关（前台运行）
hermes gateway start --port 8080

# 停止网关
hermes gateway stop
```

## 环境变量

| 变量 | 说明 |
|------|------|
| `OPENAI_API_KEY` | OpenAI API 密钥 |
| `HERMES_OPENAI_API_KEY` | OpenAI API 密钥（备选） |
| `HERMES_DEFAULT_MODEL` | 默认模型 |
| `HERMES_TELEGRAM_BOT_TOKEN` | Telegram Bot Token |
| `HERMES_TELEGRAM_VERIFY_TOKEN` | Telegram Webhook 验证 Token |
| `HERMES_WECOM_CORP_ID` | WeCom 企业 ID |
| `HERMES_WECOM_AGENT_ID` | WeCom 应用 Agent ID |
| `HERMES_WECOM_TOKEN` | WeCom Webhook Token |
| `HERMES_WECOM_AES_KEY` | WeCom AES 密钥 |

## 配置文件

配置文件位置：`~/.config/hermes-agent/config.toml`（符合 XDG 标准）

示例配置：
```toml
[defaults]
model = "openai/gpt-4o"
tools_enabled = true

[gateway]
host = "0.0.0.0"
port = 8080

[[gateway.platforms.telegram]]
bot_token = "your-bot-token"
verify_token = "your-verify-token"

[[gateway.platforms.wecom]]
corp_id = "your-corp-id"
agent_id = "your-agent-id"
token = "your-token"
aes_key = "your-aes-key"
```

## 可用模型

| 模型 ID | 描述 |
|---------|------|
| `openai/gpt-4o` | OpenAI GPT-4o - 最强能力 |
| `openai/gpt-4-turbo` | OpenAI GPT-4 Turbo - 更快更便宜 |
| `openai/gpt-3.5-turbo` | OpenAI GPT-3.5 Turbo - 最快最便宜 |
| `anthropic/claude-3-5-sonnet-20241022` | Anthropic Claude 3.5 Sonnet |
| `anthropic/claude-3-5-haiku-20241022` | Anthropic Claude 3.5 Haiku |

## 数据存储

- 会话数据：`hermes.db`（SQLite）
- 技能目录：`~/.hermes/skills/`
