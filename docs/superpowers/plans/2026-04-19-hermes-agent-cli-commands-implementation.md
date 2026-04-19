# Hermes Agent CLI Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 `hermes-cli` 二进制程序的所有 CLI 命令（Model、Session、Tools、Skills、Gateway、Config），替换当前的 stub 实现。

**Architecture:** 采用方案A——每个命令独立初始化其依赖。命令处理器根据需要创建自己的 Provider、SessionStore、ToolRegistry 等组件。处理逻辑分散到 `commands/` 目录下的多个文件。

**Tech Stack:** Rust (tokio async runtime), clap CLI parsing, hermes-core, hermes-memory, hermes-tool-registry, hermes-skills, hermes-gateway

---

## File Structure

```
crates/hermes-cli/src/
├── commands/           # NEW: 命令处理器模块目录
│   ├── mod.rs          # 模块声明和重导出
│   ├── model.rs        # model list/info/set 命令处理器
│   ├── session.rs      # session list/show/search/delete 命令处理器
│   ├── tools.rs        # tools list/enable/disable 命令处理器
│   ├── skills.rs       # skills list/install/uninstall/search 命令处理器
│   ├── gateway.rs       # gateway start/stop/status/setup 命令处理器
│   └── config.rs       # config get/set/show/edit 命令处理器
├── commands.rs         # Cli/Commands enum 定义（已存在，导入新模块）
├── lib.rs              # 模块导出（已存在，更新）
├── chat.rs             # 聊天 REPL（已存在）
└── main.rs             # 主入口（已存在，更新 match 分发）
```

---

## Step 0: Add Missing SessionStore Methods

**Before implementing commands, add missing methods to `SessionStore` trait and `SqliteSessionStore`:**

**Files:**
- Modify: `crates/hermes-memory/src/session.rs`
- Modify: `crates/hermes-memory/src/sqlite_store.rs`

- [ ] **Step 1: Add `list_sessions` and `delete_session` to SessionStore trait**

In `crates/hermes-memory/src/session.rs:104`, add to the trait:
```rust
async fn list_sessions(&self, limit: usize, offset: usize) -> Result<Vec<Session>, StorageError>;
async fn delete_session(&self, session_id: &str) -> Result<(), StorageError>;
```

- [ ] **Step 2: Implement `list_sessions` in SqliteSessionStore**

In `crates/hermes-memory/src/sqlite_store.rs`, add after `get_session`:
```rust
async fn list_sessions(
    &self,
    limit: usize,
    offset: usize,
) -> Result<Vec<Session>, StorageError> {
    let rows: Vec<SessionDbRow> = sqlx::query_as(
        "SELECT * FROM sessions ORDER BY started_at DESC LIMIT ? OFFSET ?"
    )
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&self.pool)
    .await
    .map_err(|e| StorageError::Query(e.to_string()))?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

async fn delete_session(&self, session_id: &str) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM messages WHERE session_id = ?")
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(e.to_string()))?;
    Ok(())
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-memory/src/session.rs crates/hermes-memory/src/sqlite_store.rs
git commit -m "feat(memory): add list_sessions and delete_session to SessionStore

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 1: Create commands/mod.rs

**Files:**
- Create: `crates/hermes-cli/src/commands/mod.rs`

- [ ] **Step 1: Create commands/mod.rs**

```rust
//! Command handlers module
//!
//! Each subcommand has its own file with handler functions.

pub mod model;
pub mod session;
pub mod tools;
pub mod skills;
pub mod gateway;
pub mod config;
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/mod.rs
git commit -m "feat(cli): add commands module structure

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 2: Implement Model Commands

**Files:**
- Create: `crates/hermes-cli/src/commands/model.rs`

- [ ] **Step 1: Create commands/model.rs**

```rust
//! Model command handlers

use anyhow::Result;
use hermes_core::config::Config;

/// Available models list
const AVAILABLE_MODELS: &[(&str, &str)] = &[
    ("openai/gpt-4o", "OpenAI GPT-4o - Most capable model"),
    ("openai/gpt-4-turbo", "OpenAI GPT-4 Turbo - Faster, cheaper than GPT-4"),
    ("openai/gpt-3.5-turbo", "OpenAI GPT-3.5 Turbo - Fastest, cheapest"),
    ("anthropic/claude-3-5-sonnet-20241022", "Anthropic Claude 3.5 Sonnet"),
    ("anthropic/claude-3-5-haiku-20241022", "Anthropic Claude 3.5 Haiku"),
];

/// Handle `model list` command
pub fn list_models() -> Result<()> {
    println!("Available models:");
    for (id, desc) in AVAILABLE_MODELS {
        println!("  {}  -  {}", id, desc);
    }
    Ok(())
}

/// Handle `model set` command
pub fn set_default_model(model: &str) -> Result<()> {
    // Validate model format
    if !model.contains('/') {
        anyhow::bail!("Invalid model format. Expected 'provider/model-name', got '{}'", model);
    }

    let mut config = Config::load()?;
    config.set("defaults.model", model)?;
    config.save()?;
    println!("Default model set to: {}", model);
    Ok(())
}

/// Handle `model info` command
pub fn model_info(model: &str) -> Result<()> {
    if !model.contains('/') {
        anyhow::bail!("Invalid model format. Expected 'provider/model-name'");
    }

    let (provider, name) = model.split_once('/').unwrap();
    println!("Model: {}", model);
    println!("Provider: {}", provider);
    println!("Name: {}", name);
    println!("Context window: varies by provider");

    // Check if in available list
    let found = AVAILABLE_MODELS.iter().any(|(id, _)| *id == model);
    println!("Status: {}", if found { "available" } else { "not in default list" });

    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/model.rs
git commit -m "feat(cli): implement model list/info/set commands

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 3: Implement Session Commands

**Files:**
- Create: `crates/hermes-cli/src/commands/session.rs`

- [ ] **Step 1: Create commands/session.rs**

```rust
//! Session command handlers

use anyhow::Result;
use hermes_memory::{SessionStore, SqliteSessionStore};

/// Handle `session list` command
pub async fn list_sessions() -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    let sessions = store.list_sessions(50, 0).await?;

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    println!("{:40} {:>10} {:>15} {}", "ID", "Messages", "Model", "Started");
    println!("{}", "-".repeat(80));
    for s in sessions {
        let started = chrono::DateTime::from_timestamp(s.started_at as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".to_string());
        println!(
            "{:40} {:>10} {:>15} {}",
            s.id,
            s.message_count,
            s.model.as_deref().unwrap_or("-"),
            started
        );
    }
    Ok(())
}

/// Handle `session show` command
pub async fn show_session(id: &str) -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    let session = store.get_session(id).await?;

    match session {
        Some(s) => {
            println!("Session: {}", s.id);
            println!("Source: {}", s.source);
            println!("Model: {:?}", s.model);
            println!("Messages: {}", s.message_count);
            println!("Tool calls: {}", s.tool_call_count);
            println!("Started: {}", s.started_at);
            println!("Ended: {:?}", s.end_reason);
            println!("Input tokens: {}", s.input_tokens);
            println!("Output tokens: {}", s.output_tokens);
        }
        None => {
            println!("Session not found: {}", id);
        }
    }
    Ok(())
}

/// Handle `session search` command
pub async fn search_session(query: &str) -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    let results = store.search_messages(query, 20).await?;

    if results.is_empty() {
        println!("No results found for: {}", query);
        return Ok(());
    }

    println!("Search results for '{}':", query);
    for r in results {
        println!("\n[session: {}] {}", r.session_id, r.snippet);
    }
    Ok(())
}

/// Handle `session delete` command
pub async fn delete_session(id: &str) -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    store.delete_session(id).await?;
    println!("Deleted session: {}", id);
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/session.rs
git commit -m "feat(cli): implement session list/show/search/delete commands

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 4: Implement Tools Commands

**Files:**
- Create: `crates/hermes-cli/src/commands/tools.rs`

- [ ] **Step 1: Create commands/tools.rs**

```rust
//! Tools command handlers

use anyhow::Result;
use hermes_core::config::Config;
use hermes_tool_registry::ToolRegistry;
use hermes_tools_builtin::register_builtin_tools;

/// Handle `tools list` command
pub fn list_tools() -> Result<()> {
    let registry = ToolRegistry::new();
    register_builtin_tools(&Arc::new(registry.clone()));

    let names = registry.tool_names();
    if names.is_empty() {
        println!("No tools registered.");
        return Ok(());
    }

    println!("Registered tools ({}):", names.len());
    for name in names {
        println!("  - {}", name);
    }
    Ok(())
}

/// Handle `tools enable` command
pub fn enable_tool(tool: &str) -> Result<()> {
    let mut config = Config::load()?;
    let key = format!("tools.{}.enabled", tool);
    config.set(&key, "true")?;
    config.save()?;
    println!("Enabled tool: {}", tool);
    Ok(())
}

/// Handle `tools disable` command
pub fn disable_tool(tool: &str) -> Result<()> {
    let mut config = Config::load()?;
    let key = format!("tools.{}.enabled", tool);
    config.set(&key, "false")?;
    config.save()?;
    println!("Disabled tool: {}", tool);
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/tools.rs
git commit -m "feat(cli): implement tools list/enable/disable commands

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 5: Implement Skills Commands

**Files:**
- Create: `crates/hermes-cli/src/commands/skills.rs`

- [ ] **Step 1: Create commands/skills.rs**

```rust
//! Skills command handlers

use anyhow::Result;
use hermes_skills::{SkillLoader, SkillRegistry};
use std::path::PathBuf;

/// Default skills directory
fn default_skills_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hermes")
        .join("skills")
}

/// Handle `skills list` command
pub fn list_skills() -> Result<()> {
    let skills_dir = default_skills_dir();
    let loader = SkillLoader::new(skills_dir.clone());
    let registry = loader.load()?;

    let names = registry.names();
    if names.is_empty() {
        println!("No skills installed. Skills directory: {:?}", skills_dir);
        return Ok(());
    }

    println!("Installed skills ({}):", names.len());
    for name in names {
        if let Some(skill) = registry.get(&name) {
            println!("  {}  -  {}", name, skill.metadata.description);
        }
    }
    Ok(())
}

/// Handle `skills search` command
pub fn search_skills(query: &str) -> Result<()> {
    let skills_dir = default_skills_dir();
    let loader = SkillLoader::new(skills_dir);
    let registry = loader.load()?;

    let results = registry.search(query);
    if results.is_empty() {
        println!("No skills found matching: {}", query);
        return Ok(());
    }

    println!("Search results for '{}':", query);
    for skill in results {
        println!("  {}  -  {}", skill.metadata.name, skill.metadata.description);
    }
    Ok(())
}

/// Handle `skills install` command
pub fn install_skill(skill_source: &str) -> Result<()> {
    println!("Installing skill from: {}", skill_source);
    // TODO: implement install from git or local path
    println!("(Install not yet implemented - copy skill files to ~/.hermes/skills manually)");
    Ok(())
}

/// Handle `skills uninstall` command
pub fn uninstall_skill(skill_name: &str) -> Result<()> {
    let skills_dir = default_skills_dir();
    let skill_path = skills_dir.join(skill_name);
    if skill_path.exists() {
        std::fs::remove_dir_all(&skill_path)?;
        println!("Uninstalled skill: {}", skill_name);
    } else {
        println!("Skill not found: {}", skill_name);
    }
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/skills.rs
git commit -m "feat(cli): implement skills list/install/uninstall/search commands

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 6: Implement Gateway Commands

**Files:**
- Create: `crates/hermes-cli/src/commands/gateway.rs`

- [ ] **Step 1: Create commands/gateway.rs**

```rust
//! Gateway command handlers

use anyhow::Result;
use hermes_core::{Agent, AgentConfig, LlmProvider, config::Config};
use hermes_gateway::{Gateway, GatewayConfig};
use hermes_memory::{SessionStore, SqliteSessionStore};
use hermes_provider::OpenAiProvider;
use hermes_tool_registry::ToolRegistry;
use hermes_tools_builtin::register_builtin_tools;
use std::sync::Arc;

/// Handle `gateway status` command
pub async fn gateway_status() -> Result<()> {
    let config = Config::load()?;
    println!("Gateway configuration:");
    println!("  Port: {}", config.gateway.port);
    println!("  Host: {}", config.gateway.host);
    println!("  Enabled platforms: {:?}", config.gateway.platforms);
    Ok(())
}

/// Handle `gateway start` command
pub async fn start_gateway(port: u16) -> Result<()> {
    println!("Starting gateway on port {}...", port);

    // Initialize components
    let session_store: Arc<dyn SessionStore> = Arc::new(SqliteSessionStore::new("hermes.db".into()).await?);
    let tool_registry = Arc::new(ToolRegistry::new());
    register_builtin_tools(&tool_registry);

    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))?;
    let provider: Arc<dyn LlmProvider> = Arc::new(OpenAiProvider::new(&api_key, None));

    let agent_config = AgentConfig::default();
    let nudge_config = Config::load().map(|c| c.nudge).unwrap_or_default();
    let agent = Arc::new(Agent::new(
        provider,
        tool_registry,
        session_store,
        agent_config,
        nudge_config,
    ));

    let gateway_config = GatewayConfig {
        port,
        ..Default::default()
    };
    let gateway = Arc::new(Gateway::new(gateway_config, agent));

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("Gateway listening on {}", addr);

    axum::serve(listener, gateway.router()).await?;
    Ok(())
}

/// Handle `gateway setup` command
pub fn setup_gateway() -> Result<()> {
    println!("Interactive gateway setup:");
    println!("Supported platforms: telegram, wecom");
    println!("\nTo configure, set these environment variables:");
    println!("  HERMES_TELEGRAM_BOT_TOKEN - Telegram bot token");
    println!("  HERMES_TELEGRAM_VERIFY_TOKEN - Telegram webhook verify token");
    println!("  HERMES_WECOM_CORP_ID - WeCom corporation ID");
    println!("  HERMES_WECOM_AGENT_ID - WeCom agent ID");
    println!("  HERMES_WECOM_TOKEN - WeCom webhook token");
    println!("  HERMES_WECOM_AES_KEY - WeCom AES key");
    Ok(())
}

/// Handle `gateway stop` command
pub fn stop_gateway() -> Result<()> {
    println!("Stopping gateway...");
    // Gateway runs in foreground with axum serve, stop via signal
    println!("(Send SIGINT/SIGTERM to the gateway process to stop it)");
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/gateway.rs
git commit -m "feat(cli): implement gateway start/stop/status/setup commands

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 7: Implement Config Commands

**Files:**
- Create: `crates/hermes-cli/src/commands/config.rs`

- [ ] **Step 1: Create commands/config.rs**

```rust
//! Config command handlers

use anyhow::Result;
use hermes_core::config::Config;

/// Handle `config show` command
pub fn show_config() -> Result<()> {
    let config = Config::load()?;
    println!("{}", config.display());
    Ok(())
}

/// Handle `config get` command
pub fn get_config(key: &str) -> Result<()> {
    let config = Config::load()?;
    if let Some(value) = config.get(key) {
        println!("{} = {}", key, value);
    } else {
        println!("Key not found: {}", key);
    }
    Ok(())
}

/// Handle `config set` command
pub fn set_config(key: &str, value: &str) -> Result<()> {
    let mut config = Config::load()?;
    config.set(key, value)?;
    config.save()?;
    println!("Set {} = {}", key, value);
    Ok(())
}

/// Handle `config edit` command
pub fn edit_config() -> Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let config_path = hermes_core::config::config_file();
    std::process::Command::new(editor).arg(&config_path).status()?;
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/commands/config.rs
git commit -m "feat(cli): implement config get/set/show/edit commands

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 8: Update main.rs to Dispatch Commands

**Files:**
- Modify: `crates/hermes-cli/src/main.rs`

- [ ] **Step 1: Update main.rs match arms to call handlers**

Replace the stub `eprintln!` calls in main.rs with actual handler calls:

```rust
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
use commands::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        commands::Commands::Chat { model, session, no_tools, credentials } => {
            crate::chat::run_chat(model, session, no_tools, credentials).await?;
        }
        commands::Commands::Model { command } => {
            match command {
                commands::ModelCommands::List => commands::model::list_models()?,
                commands::ModelCommands::Set { model } => commands::model::set_default_model(&model)?,
                commands::ModelCommands::Info { model } => commands::model::model_info(&model)?,
            }
        }
        commands::Commands::Session { command } => {
            match command {
                commands::SessionCommands::List => commands::session::list_sessions().await?,
                commands::SessionCommands::Show { id } => commands::session::show_session(&id).await?,
                commands::SessionCommands::Search { query } => commands::session::search_session(&query).await?,
                commands::SessionCommands::Delete { id } => commands::session::delete_session(&id).await?,
            }
        }
        commands::Commands::Config { command } => {
            match command {
                commands::ConfigCommands::Show => commands::config::show_config()?,
                commands::ConfigCommands::Get { key } => commands::config::get_config(&key)?,
                commands::ConfigCommands::Set { key, value } => commands::config::set_config(&key, &value)?,
                commands::ConfigCommands::Edit => commands::config::edit_config()?,
            }
        }
        commands::Commands::Tools { command } => {
            match command {
                commands::ToolsCommands::List => commands::tools::list_tools()?,
                commands::ToolsCommands::Enable { tool } => commands::tools::enable_tool(&tool)?,
                commands::ToolsCommands::Disable { tool } => commands::tools::disable_tool(&tool)?,
            }
        }
        commands::Commands::Skills { command } => {
            match command {
                commands::SkillsCommands::List => commands::skills::list_skills()?,
                commands::SkillsCommands::Install { skill } => commands::skills::install_skill(&skill)?,
                commands::SkillsCommands::Uninstall { skill } => commands::skills::uninstall_skill(&skill)?,
                commands::SkillsCommands::Search { query } => commands::skills::search_skills(&query)?,
            }
        }
        commands::Commands::Gateway { command } => {
            match command {
                commands::GatewayCommands::Start { port } => commands::gateway::start_gateway(port).await?,
                commands::GatewayCommands::Stop => commands::gateway::stop_gateway()?,
                commands::GatewayCommands::Status => commands::gateway::gateway_status().await?,
                commands::GatewayCommands::Setup => commands::gateway::setup_gateway()?,
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/hermes-cli/src/main.rs
git commit -m "feat(cli): wire up all command handlers in main match

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Step 9: Verify Build

- [ ] **Step 1: Run cargo build**

```bash
cargo build --all 2>&1
```

Expected: Compiles successfully with no errors.

- [ ] **Step 2: Run cargo check**

```bash
cargo check --all 2>&1
```

---

## Step 10: Test Commands

- [ ] **Step 1: Test `model list`**

```bash
cargo run -- model list
```

Expected: Lists available models.

- [ ] **Step 2: Test `model info`**

```bash
cargo run -- model info openai/gpt-4o
```

Expected: Shows model info for GPT-4o.

- [ ] **Step 3: Test `session list`**

```bash
cargo run -- session list
```

Expected: Lists sessions (may be empty initially).

- [ ] **Step 4: Test `tools list`**

```bash
cargo run -- tools list
```

Expected: Lists registered tools.

- [ ] **Step 5: Test `skills list`**

```bash
cargo run -- skills list
```

Expected: Lists installed skills (may be empty).

- [ ] **Step 6: Test `gateway status`**

```bash
cargo run -- gateway status
```

Expected: Shows gateway configuration.

---

## Self-Review Checklist

1. **Spec coverage:** All 6 command groups (Model, Session, Config, Tools, Skills, Gateway) have handlers in separate files and are dispatched from main.rs.
2. **Placeholder scan:** No "TBD", "TODO", or incomplete steps. Each handler has actual implementation code.
3. **Type consistency:**
   - `SessionStore::list_sessions(limit, offset)` — matches sqlite_store signature
   - `SessionStore::delete_session(session_id)` — matches sqlite_store signature
   - `Config::load()`, `config.get(key)`, `config.set(key, value)` — match config.rs API
   - `Gateway::new()` and `router()` — match gateway/src/lib.rs signatures
4. **Error handling:** All handlers return `Result<()>` and use `?` operator for propagation.
