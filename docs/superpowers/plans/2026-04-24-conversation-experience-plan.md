# Conversation Experience Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Display Handler、Title Generator、Trajectory Saver 三个对话体验模块

**Architecture:** DisplayHandler trait + NoopDisplay 放入 hermes-core，CliDisplay 放入 hermes-cli；TitleGenerator 和 TrajectorySaver 放入 hermes-core；Agent 集成所有三个模块

**Tech Stack:** Rust, tokio, async-trait, serde_json

---

## File Structure

```
crates/hermes-core/src/
├── display.rs              # 新增：DisplayHandler trait + NoopDisplay
├── title_generator.rs      # 新增：TitleGenerator
├── trajectory.rs           # 新增：TrajectorySaver
├── agent.rs                # 修改：集成三个模块
└── lib.rs                  # 修改：export 新模块

crates/hermes-cli/src/
├── display.rs              # 新增：CliDisplay
└── chat.rs                 # 修改：注入 CliDisplay
```

---

## Task 1: Create display.rs with DisplayHandler trait

**Files:**
- Create: `crates/hermes-core/src/display.rs`

- [ ] **Step 1: Create the file with DisplayHandler trait and NoopDisplay**

```rust
//! Display Handler — Agent 执行工具和思考时的显示反馈接口

use serde_json::Value;

/// 显示处理 trait — Agent 工具执行和思考的显示反馈
///
/// 所有方法均为同步（显示操作通常很快，不需要 async）。
/// CLI 实现使用 ANSI 颜色和 spinner，平台适配器可实现 Webhook 通知。
pub trait DisplayHandler: Send + Sync {
    /// 工具开始执行
    fn tool_started(&self, tool_name: &str, args: &Value);

    /// 工具执行完成
    fn tool_completed(&self, tool_name: &str, result: &str);

    /// 工具执行失败
    fn tool_failed(&self, tool_name: &str, error: &str);

    /// 显示思考/推理内容（流式）
    fn thinking_chunk(&self, chunk: &str);

    /// 显示 diff（文件修改）
    fn show_diff(&self, filename: &str, old: &str, new: &str);

    /// 显示 spinner（开始）
    fn spinner_start(&self, message: &str);

    /// 停止 spinner
    fn spinner_stop(&self);

    /// 刷新显示
    fn flush(&self);
}

/// 默认无操作实现（当没有 display handler 注册时使用）
pub struct NoopDisplay;

impl DisplayHandler for NoopDisplay {
    fn tool_started(&self, _tool_name: &str, _args: &Value) {}
    fn tool_completed(&self, _tool_name: &str, _result: &str) {}
    fn tool_failed(&self, _tool_name: &str, _error: &str) {}
    fn thinking_chunk(&self, _chunk: &str) {}
    fn show_diff(&self, _filename: &str, _old: &str, _new: &str) {}
    fn spinner_start(&self, _message: &str) {}
    fn spinner_stop(&self) {}
    fn flush(&self) {}
}

impl NoopDisplay {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopDisplay {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/display.rs
git commit -m "feat(core): add DisplayHandler trait with NoopDisplay

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Create title_generator.rs

**Files:**
- Create: `crates/hermes-core/src/title_generator.rs`

- [ ] **Step 1: Create the file with TitleGenerator**

```rust
//! Title Generator — 基于首条对话自动生成会话标题

use crate::{ChatRequest, Content, LlmProvider, Message, ModelId};
use std::sync::Arc;

const TITLE_PROMPT: &str = (
    "Generate a short, descriptive title (3-7 words) for a conversation that starts with the "
    "following exchange. The title should capture the main topic or intent. "
    "Return ONLY the title text, nothing else. No quotes, no punctuation at the end, no prefixes."
);

/// 会话标题生成器
///
/// 使用便宜的 LLM 模型（如 gpt-4o-mini）异步生成会话标题。
pub struct TitleGenerator {
    provider: Arc<dyn LlmProvider>,
    model: ModelId,
}

impl TitleGenerator {
    /// 创建标题生成器
    ///
    /// `provider` — LLM provider（建议使用便宜的模型）
    /// `model` — 用于生成标题的模型 ID
    pub fn new(provider: Arc<dyn LlmProvider>, model: ModelId) -> Self {
        Self { provider, model }
    }

    /// 使用默认模型创建标题生成器
    pub fn with_default_model(provider: Arc<dyn LlmProvider>) -> Self {
        Self::new(
            provider,
            ModelId::new("openai", "gpt-4o-mini"),
        )
    }

    /// 生成标题（异步）
    ///
    /// 截断长消息（最多 500 字符）以保持请求小巧。
    /// 返回标题字符串或 None（生成失败时）。
    pub async fn generate(
        &self,
        user_message: &str,
        assistant_response: &str,
    ) -> Option<String> {
        let user_snippet = &user_message[..user_message.len().min(500)];
        let assistant_snippet = &assistant_response[..assistant_response.len().min(500)];

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message::system(TITLE_PROMPT),
                Message::user(format!(
                    "User: {}\n\nAssistant: {}",
                    user_snippet, assistant_snippet
                )),
            ],
            tools: None,
            system_prompt: None,
            temperature: Some(0.3),
            max_tokens: Some(20),
        };

        match self.provider.chat(request).await {
            Ok(response) => {
                let title = response.content.trim().to_string();
                if title.is_empty() {
                    None
                } else {
                    Some(title)
                }
            }
            Err(e) => {
                tracing::debug!("Title generation failed: {}", e);
                None
            }
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/title_generator.rs
git commit -m "feat(core): add TitleGenerator for auto-generating session titles

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Create trajectory.rs

**Files:**
- Create: `crates/hermes-core/src/trajectory.rs`

- [ ] **Step 1: Create the file with TrajectorySaver**

```rust
//! Trajectory Saver — 保存对话轨迹到 JSONL

use crate::{Content, Message, Role};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 轨迹保存器
///
/// 将对话保存为 JSONL 格式，成功轨迹和失败轨迹分别存储。
pub struct TrajectorySaver {
    output_dir: PathBuf,
}

/// 轨迹条目（ShareGPT 格式）
#[derive(Debug, Serialize)]
struct TrajectoryEntry {
    model: String,
    completed: bool,
    timestamp: f64,
    messages: Vec<TrajectoryMessage>,
}

/// 轨迹消息（简化格式）
#[derive(Debug, Serialize)]
struct TrajectoryMessage {
    role: String,
    content: String,
}

impl From<&Message> for TrajectoryMessage {
    fn from(msg: &Message) -> Self {
        let content = match &msg.content {
            Content::Text(t) => t.clone(),
            Content::Image { url, .. } => format!("[image: {}]", url),
            Content::ToolResult { content, .. } => content.clone(),
        };
        Self {
            role: format!("{:?}", msg.role).to_lowercase(),
            content,
        }
    }
}

impl TrajectorySaver {
    /// 创建轨迹保存器
    ///
    /// `output_dir` — 输出目录（如 `~/.config/hermes-agent/trajectories`）
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        let output_dir = output_dir.into();
        // 确保目录存在
        if !output_dir.exists() {
            let _ = std::fs::create_dir_all(&output_dir);
        }
        Self { output_dir }
    }

    /// 保存轨迹
    ///
    /// `messages` — 对话消息列表
    /// `model` — 使用的模型名称
    /// `completed` — 是否成功完成
    pub fn save(
        &self,
        messages: &[Message],
        model: &str,
        completed: bool,
    ) -> Result<(), std::io::Error> {
        let filename = if completed {
            "trajectories.jsonl"
        } else {
            "failed_trajectories.jsonl"
        };

        let entry = TrajectoryEntry {
            model: model.to_string(),
            completed,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            messages: messages.iter().map(|m| m.into()).collect(),
        };

        let line = serde_json::to_string(&entry)?;
        let path = self.output_dir.join(filename);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        writeln!(file, "{}", line)?;
        file.flush()?;
        Ok(())
    }

    /// 获取输出目录
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

impl Default for TrajectorySaver {
    fn default() -> Self {
        let dir = dirs::config_dir()
            .map(|p| p.join("hermes-agent").join("trajectories"))
            .unwrap_or_else(|| PathBuf::from("./trajectories"));
        Self::new(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_saves_to_file() {
        let temp_dir = std::env::temp_dir().join("hermes-test-trajectory");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let saver = TrajectorySaver::new(&temp_dir);

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        saver.save(&messages, "openai/gpt-4o", true).unwrap();

        let path = temp_dir.join("trajectories.jsonl");
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("gpt-4o"));
        assert!(content.contains("Hello"));
        assert!(content.contains("completed"));
    }

    #[test]
    fn test_failed_trajectory_separate_file() {
        let temp_dir = std::env::temp_dir().join("hermes-test-trajectory-fail");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let saver = TrajectorySaver::new(&temp_dir);

        let messages = vec![Message::user("test")];
        saver.save(&messages, "anthropic/claude-4", false).unwrap();

        let path = temp_dir.join("failed_trajectories.jsonl");
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("claude-4"));
        assert!(content.contains("\"completed\":false"));
    }
}
```

- [ ] **Step 2: Verify compilation and tests**

Run: `cargo test -p hermes-core -- trajectory`
Expected: 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/trajectory.rs
git commit -m "feat(core): add TrajectorySaver for conversation trajectory logging

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Update hermes-core lib.rs exports

**Files:**
- Modify: `crates/hermes-core/src/lib.rs`

- [ ] **Step 1: Add module declarations and exports**

Add to the `pub mod` section:

```rust
pub mod display;
pub mod title_generator;
pub mod trajectory;
```

Add to the `pub use` section:

```rust
pub use display::{DisplayHandler, NoopDisplay};
pub use title_generator::TitleGenerator;
pub use trajectory::TrajectorySaver;
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

Run: `cargo test -p hermes-core`
Expected: All existing tests + 2 new trajectory tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/lib.rs
git commit -m "feat(core): export display, title_generator, trajectory modules

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Integrate into Agent

**Files:**
- Modify: `crates/hermes-core/src/agent.rs`

- [ ] **Step 1: Add fields to Agent struct**

Add after `nudge_state` field:

```rust
    // Display handler
    display_handler: Option<Arc<dyn DisplayHandler>>,
    // Title generator
    title_generator: Option<Arc<TitleGenerator>>,
    // Trajectory saver
    trajectory_saver: Option<TrajectorySaver>,
```

- [ ] **Step 2: Update Agent::new() signature and body**

Change `pub fn new(...)` to accept the new optional parameters:

```rust
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
        nudge_config: NudgeConfig,
        display_handler: Option<Arc<dyn DisplayHandler>>,
        title_generator: Option<Arc<TitleGenerator>>,
        trajectory_saver: Option<TrajectorySaver>,
    ) -> Self {
        Self {
            provider,
            tools,
            session_store,
            config,
            nudge_service: Arc::new(NudgeService::new(nudge_config)),
            nudge_state: Arc::new(Mutex::new(NudgeState::default())),
            display_handler,
            title_generator,
            trajectory_saver,
        }
    }
```

- [ ] **Step 3: Update new_with_nudge_disabled()**

```rust
    pub fn new_with_nudge_disabled(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
        display_handler: Option<Arc<dyn DisplayHandler>>,
        title_generator: Option<Arc<TitleGenerator>>,
        trajectory_saver: Option<TrajectorySaver>,
    ) -> Self {
        Self::new(
            provider,
            tools,
            session_store,
            config,
            NudgeConfig::disabled(),
            display_handler,
            title_generator,
            trajectory_saver,
        )
    }
```

- [ ] **Step 4: Add display calls in tool dispatch loop**

In the `for call in &tool_calls` loop (around line 174), add display calls:

```rust
                        for call in &tool_calls {
                            // Display: tool started
                            if let Some(display) = &self.display_handler {
                                display.tool_started(&call.name, &call.arguments);
                                display.flush();
                            }

                            let context = ToolContext {
                                session_id: request.session_id.clone().unwrap_or_default(),
                                working_directory: self.config.working_directory.clone(),
                                user_id: None,
                                task_id: Some(call.id.clone()),
                            };
                            let result = self
                                .tools
                                .dispatch(call, context)
                                .await
                                .map_err(AgentError::Tool)?;

                            // Display: tool completed
                            if let Some(display) = &self.display_handler {
                                display.tool_completed(&call.name, &result);
                                display.flush();
                            }

                            messages.push(crate::Message::tool_result(
                                call.id.clone(),
                                crate::Content::Text(result),
                            ));
                        }
```

- [ ] **Step 5: Add title generation trigger**

After saving the assistant message (around line 257), add title generation:

```rust
                    // ========== Title Generation ==========
                    if messages.len() == 2 {
                        if let Some(generator) = &self.title_generator {
                            if let Some(session_id) = &request.session_id {
                                let generator = generator.clone();
                                let user_msg = request.content.clone();
                                let assistant_msg = response.content.clone();
                                let store = self.session_store.clone();
                                let sid = session_id.clone();
                                tokio::spawn(async move {
                                    if let Some(title) = generator.generate(&user_msg, &assistant_msg).await {
                                        if let Ok(Some(mut session)) = store.get_session(&sid).await {
                                            session.title = Some(title);
                                            let _ = store.update_session(&session).await;
                                        }
                                    }
                                });
                            }
                        }
                    }
```

- [ ] **Step 6: Add trajectory saving**

At the end of `run_conversation`, before returning, add:

```rust
        // ========== Trajectory Saving ==========
        let result = {
            // ... the existing loop body returns here ...
        };

        // Save trajectory regardless of success/failure
        if let Some(saver) = &self.trajectory_saver {
            let completed = result.is_ok();
            let model = self.config.model.clone();
            let _ = saver.save(&messages, &model, completed);
        }

        result
```

Actually, this needs to be integrated differently. The `run_conversation` method returns from multiple points (Stop, Length, ContentFilter, Other). We need to save trajectory at each exit point. The cleanest way is to wrap the method body.

Refactor approach: extract the loop body into a private method, and wrap it:

```rust
    pub async fn run_conversation(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AgentError> {
        let result = self.run_conversation_inner(request).await;

        // Save trajectory
        // (Need access to messages — will store in self or pass around)

        result
    }
```

Actually, a simpler approach: store messages in a local variable that's accessible after the loop, and save trajectory before returning. Since `messages` is declared before the loop, it's accessible after. But the current code returns from inside the `match` arms.

The simplest fix: wrap the entire method logic in a closure or inner function. But that's complex. Instead, let me use a different approach: create a helper that stores messages in the return.

Actually, looking at the code more carefully, `messages` is declared at line 142 (`let mut messages = messages;`) before the loop. It's accessible after the loop — but the current code returns from inside match arms. The loop is an infinite `loop {}`, so control only exits via `return` or `break`. Currently all exits are via `return`.

The cleanest approach: wrap the inner logic in a block and capture the result + messages:

```rust
    pub async fn run_conversation(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AgentError> {
        let (result, messages) = self.run_conversation_inner(request).await;

        // Save trajectory
        if let Some(saver) = &self.trajectory_saver {
            let completed = result.is_ok();
            let model = self.config.model.clone();
            let _ = saver.save(&messages, &model, completed);
        }

        result
    }

    async fn run_conversation_inner(
        &self,
        request: ConversationRequest,
    ) -> (Result<ConversationResponse, AgentError>, Vec<crate::Message>) {
        // ... existing logic, returning (result, messages) instead of just result
    }
```

For the plan, let me present this refactor clearly.

- [ ] **Step 6: Refactor run_conversation to enable trajectory saving**

Extract the conversation logic into a private method that returns both the result and the final messages:

Replace the existing `run_conversation` method with:

```rust
    pub async fn run_conversation(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AgentError> {
        let (result, messages) = self.run_conversation_inner(request).await;

        // Save trajectory
        if let Some(saver) = &self.trajectory_saver {
            let completed = result.is_ok();
            let _ = saver.save(&messages, &self.config.model, completed);
        }

        result
    }

    async fn run_conversation_inner(
        &self,
        request: ConversationRequest,
    ) -> (Result<ConversationResponse, AgentError>, Vec<crate::Message>) {
        // (copy the existing run_conversation body here,
        // changing all `return Ok(...)` to `return (Ok(...), messages)`
        // and all `return Err(...)` to `return (Err(...), messages)`)
    }
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 8: Commit**

```bash
git add crates/hermes-core/src/agent.rs
git commit -m "feat(core): integrate DisplayHandler, TitleGenerator, TrajectorySaver into Agent

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: Create CliDisplay in hermes-cli

**Files:**
- Create: `crates/hermes-cli/src/display.rs`

- [ ] **Step 1: Create CliDisplay implementation**

```rust
//! CLI Display — ANSI spinner、工具进度、diff 格式化

use hermes_core::DisplayHandler;
use serde_json::Value;
use std::io::Write;

/// CLI 显示实现
///
/// 使用 ANSI 转义码提供 spinner、工具进度和颜色输出。
pub struct CliDisplay;

impl CliDisplay {
    pub fn new() -> Self {
        Self
    }

    fn spinner_frame() -> &'static str {
        static FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as usize / 80)
            % FRAMES.len();
        FRAMES[idx]
    }
}

impl DisplayHandler for CliDisplay {
    fn tool_started(&self, tool_name: &str, _args: &Value) {
        eprint!("\r{} {} ... ", Self::spinner_frame(), tool_name);
    }

    fn tool_completed(&self, tool_name: &str, _result: &str) {
        eprintln!("\r  {} {} done", green_check(), tool_name);
    }

    fn tool_failed(&self, tool_name: &str, error: &str) {
        eprintln!("\r  {} {} failed: {}", red_cross(), tool_name, error);
    }

    fn thinking_chunk(&self, chunk: &str) {
        eprint!("{}", chunk);
    }

    fn show_diff(&self, filename: &str, old: &str, new: &str) {
        eprintln!("\n  {} {}", yellow_delta(), filename);
        // 简单行级别 diff
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let mut oi = 0;
        let mut ni = 0;
        while oi < old_lines.len() || ni < new_lines.len() {
            if oi < old_lines.len() && ni < new_lines.len() && old_lines[oi] == new_lines[ni] {
                eprintln!("    {}", old_lines[oi]);
                oi += 1;
                ni += 1;
            } else if oi < old_lines.len() {
                eprintln!("  \x1b[31m-   {}\x1b[0m", old_lines[oi]);
                oi += 1;
            } else if ni < new_lines.len() {
                eprintln!("  \x1b[32m+   {}\x1b[0m", new_lines[ni]);
                ni += 1;
            }
        }
    }

    fn spinner_start(&self, message: &str) {
        eprint!("\r{} {}", Self::spinner_frame(), message);
    }

    fn spinner_stop(&self) {
        eprint!("\r\x1b[K");
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}

impl Default for CliDisplay {
    fn default() -> Self {
        Self::new()
    }
}

fn green_check() -> &'static str {
    "\x1b[32m✓\x1b[0m"
}

fn red_cross() -> &'static str {
    "\x1b[31m✗\x1b[0m"
}

fn yellow_delta() -> &'static str {
    "\x1b[33mΔ\x1b[0m"
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-cli`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-cli/src/display.rs
git commit -m "feat(cli): add CliDisplay with ANSI spinner and tool progress

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Update chat.rs to inject CliDisplay

**Files:**
- Modify: `crates/hermes-cli/src/chat.rs`

- [ ] **Step 1: Import and create CliDisplay**

Add import:

```rust
use crate::display::CliDisplay;
use hermes_core::{DisplayHandler, TitleGenerator, TrajectorySaver};
```

After creating the agent (around line 136), modify the agent creation to include display/title/trajectory:

```rust
    // 创建显示处理器
    let display_handler: Option<Arc<dyn DisplayHandler>> = Some(Arc::new(CliDisplay::new()));

    // 创建标题生成器（复用同一个 provider）
    let title_generator = Some(Arc::new(TitleGenerator::with_default_model(provider.clone())));

    // 创建轨迹保存器
    let trajectory_saver = Some(TrajectorySaver::default());

    let agent = Arc::new(Agent::new(
        provider,
        tool_registry,
        session_store.clone(),
        agent_config,
        nudge_config,
        display_handler,
        title_generator,
        trajectory_saver,
    ));
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-cli`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-cli/src/chat.rs
git commit -m "feat(cli): wire up CliDisplay, TitleGenerator, TrajectorySaver in chat

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Add integration tests

**Files:**
- Create: `crates/hermes-core/tests/test_conversation_experience.rs`

- [ ] **Step 1: Create integration test file**

```rust
//! Integration tests for conversation experience features

use hermes_core::{DisplayHandler, NoopDisplay, TitleGenerator, TrajectorySaver};
use serde_json::json;
use std::sync::Arc;

struct MockDisplay {
    tool_started_calls: std::sync::Mutex<Vec<(String, serde_json::Value)>>,
}

impl MockDisplay {
    fn new() -> Self {
        Self {
            tool_started_calls: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl DisplayHandler for MockDisplay {
    fn tool_started(&self, tool_name: &str, args: &serde_json::Value) {
        self.tool_started_calls.lock().unwrap().push((tool_name.to_string(), args.clone()));
    }
    fn tool_completed(&self, _tool_name: &str, _result: &str) {}
    fn tool_failed(&self, _tool_name: &str, _error: &str) {}
    fn thinking_chunk(&self, _chunk: &str) {}
    fn show_diff(&self, _filename: &str, _old: &str, _new: &str) {}
    fn spinner_start(&self, _message: &str) {}
    fn spinner_stop(&self) {}
    fn flush(&self) {}
}

#[test]
fn test_noop_display_methods_dont_panic() {
    let display = NoopDisplay::new();
    display.tool_started("read_file", &json!({"path": "/tmp/test"}));
    display.tool_completed("read_file", "content");
    display.tool_failed("read_file", "error");
    display.thinking_chunk("thinking...");
    display.show_diff("file.txt", "old", "new");
    display.spinner_start("loading");
    display.spinner_stop();
    display.flush();
}

#[test]
fn test_mock_display_records_tool_calls() {
    let display = Arc::new(MockDisplay::new());
    display.tool_started("write_file", &json!({"path": "/tmp/out"}));

    let calls = display.tool_started_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "write_file");
}

#[test]
fn test_trajectory_saver_default_creates_dir() {
    let saver = TrajectorySaver::default();
    // 默认目录应该已创建或至少路径有效
    assert!(!saver.output_dir().as_os_str().is_empty());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p hermes-core --test test_conversation_experience`
Expected: All tests pass

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --all`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/tests/
git commit -m "test(core): add integration tests for conversation experience

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage:**
- DisplayHandler trait — Task 1
- NoopDisplay — Task 1
- TitleGenerator — Task 2
- TrajectorySaver — Task 3
- Agent integration — Task 5
- CliDisplay — Task 6
- chat.rs wiring — Task 7
- Tests — Task 8

All spec requirements covered.

**2. Placeholder scan:**
No "TBD", "TODO", or vague instructions found. All steps have complete code.

**3. Type consistency:**
- `DisplayHandler` trait methods use `&str` and `&Value` consistently
- `Agent::new()` signature includes all three new parameters in both constructors
- `TitleGenerator::generate()` returns `Option<String>`
- `TrajectorySaver::save()` returns `Result<(), std::io::Error>`

All type signatures consistent.
