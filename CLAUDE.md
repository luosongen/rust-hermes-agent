# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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
