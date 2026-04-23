# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.
## 基础交互规则

### 沟通约定

1. **语言要求**：所有回答与讨论使用**中文**。
2. **解释性注释**：生成的代码中，关键节点和复杂逻辑处需添加**中文注释**。
3. **代码聚合**：当生成代码超过20行时，主动提示考虑模块化或重构，评估代码颗粒度是否合适。


**权衡说明**：这些准则更偏向谨慎而非速度。处理简单任务时请自行判断。

## 1. 先思考再编码
**不臆测、不隐瞒困惑、明确权衡**
开始实现前：
- 明确陈述你的假设。如有不确定，主动提问。
- 若存在多种解读，全部列出——不私下选定。
- 若有更简单的方案，主动说明。必要时提出异议。
- 若内容不清晰，立即停止。明确指出困惑点，主动提问。

## 2. 简洁优先
**用最少的代码解决问题。不做任何推测性设计。**
- 不添加需求之外的功能。
- 单次使用的代码不做抽象封装。
- 不添加未被要求的“灵活性”或“可配置性”。
- 不为不可能的场景做异常处理。
- 若写了200行代码却能用50行实现，重写。

自我检查：“资深工程师会认为这过于复杂吗？”如果是，简化。

## 3. 精准修改
**只修改必要内容。只清理自己造成的冗余。**
修改现有代码时：
- 不“优化”相邻代码、注释或格式。
- 不重构无问题的代码。
- 遵循现有代码风格，即便你有不同写法。
- 若发现无关的死代码，仅告知——不删除。

修改产生冗余时：
- 移除因**你的修改**而不再使用的导入、变量、函数。
- 除非被要求，否则不删除原有死代码。

检验标准：每一行修改都应直接对应需求。

## 4. 目标导向执行
**定义成功标准。循环验证直至通过。**
将任务转化为可验证目标：
- “添加校验”→“编写无效输入的测试用例，使其通过”
- “修复bug”→“编写复现bug的测试用例，使其通过”
- “重构X”→“确保重构前后测试均通过”

多步骤任务，简述计划：
```
1. [步骤] → 验证：[检查项]
2. [步骤] → 验证：[检查项]
3. [步骤] → 验证：[检查项]
```

清晰的成功标准可让你独立循环验证。模糊标准（“让它运行”）需要反复确认。

---

**准则生效的表现**：代码差异中无意义修改更少、因过度复杂导致的重写更少、实现前就提出澄清问题，而非出错后补救。
 

## Build Commands

```bash
# Build all crates
cargo build --all

# Run tests (all crates)
cargo test --all

# Run tests for specific crate
cargo test -p hermes-core

# Run tests with output
cargo test --all -- --nocapture

# Check compilation
cargo check --all

# Lint
cargo clippy --all
```

## Architecture

rust-hermes-agent is a Rust CLI tool for AI conversation with multi-provider support, tool execution, and messaging platform adapters.

### Core Flow

```
CLI (hermes-cli) → Agent (hermes-core) → LlmProvider (hermes-provider)
                ↓
         ToolDispatcher → ToolRegistry → Tools (hermes-tools-builtin)
                ↓
         SessionStore (hermes-memory) → SQLite
```

### Key Crates

| Crate | Responsibility |
|-------|----------------|
| `hermes-core` | `Agent`, `ConversationRequest/Response`, `LlmProvider` trait, `ToolDispatcher` trait, Config |
| `hermes-cli` | CLI parsing (clap), interactive REPL (`chat.rs`) |
| `hermes-memory` | `SessionStore` trait, `SqliteSessionStore` implementation |
| `hermes-provider` | `OpenAiProvider` implementation |
| `hermes-tool-registry` | `ToolRegistry`, `Tool` trait |
| `hermes-tools-builtin` | Built-in tools: `ReadFileTool`, `WriteFileTool`, `TerminalTool` |
| `hermes-gateway` | HTTP server for webhooks, `PlatformAdapter` trait |
| `hermes-platform-telegram` | Telegram webhook adapter |
| `hermes-platform-wecom` | WeCom webhook adapter |

### Important Traits

- **`LlmProvider`**: Implemented by providers (OpenAI, etc.). `chat(request: ChatRequest) -> Result<ChatResponse>`
- **`ToolDispatcher`**: `get_definitions()` returns tool schemas, `dispatch()` executes tools
- **`Tool`**: `name()`, `description()`, `parameters()`, `execute()`
- **`SessionStore`**: `create_session()`, `append_message()`, `get_messages()`
- **`PlatformAdapter`**: `verify_webhook()`, `parse_inbound()`, `send_outbound()`

### Config System

Config location: `~/.config/hermes-agent/config.toml` (XDG compliant)

Priority: CLI flags > Environment variables (`HERMES_*`) > Config file > Defaults

Key env vars: `HERMES_DEFAULT_MODEL`, `HERMES_OPENAI_API_KEY`, `HERMES_TELEGRAM_BOT_TOKEN`

### Model ID Format

Format: `provider/model-name` (e.g., `openai/gpt-4o`, `anthropic/claude-3-5-sonnet-20241022`)

### Provider Implementation

Providers are defined in `hermes-provider/src/openai.rs` and re-exported via `hermes-provider/src/lib.rs`. To add a new provider, implement the `LlmProvider` trait from `hermes-core`.

### Platform Adapter Notes

- `verify_webhook()` is **sync** (not `async`) because it only checks query params
- `parse_inbound()` is **async** and parses platform-specific formats into `InboundMessage`
- WeCom uses AES-256-CBC decryption with manual CBC implementation
- Telegram uses simple token verification

### Testing

Integration tests exist in `crates/*/tests/`. Key test files:
- `crates/hermes-platform-telegram/tests/test_telegram.rs`
- `crates/hermes-platform-wecom/tests/test_wecom.rs`
- `crates/hermes-memory/tests/test_store.rs`
- `crates/hermes-tool-registry/tests/test_registry.rs`

### Async Runtime

Uses `tokio` with `parking_lot` for internal locking (not tokio's own locks). Axum 0.8 for HTTP.

## Project Location

Repository: `/Users/Rowe/ai-projects/rust-hermes-agent`
