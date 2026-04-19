# Agent Nudge System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Rust 版 Hermes Agent 的 Nudge System，完整复刻 Python 版功能：每 N 轮对话后 spawn background review agent 询问是否保存用户信息和创建技能。

**Architecture:** 创建独立的 `nudge.rs` 模块，包含 `NudgeConfig`、`NudgeState`、`NudgeTrigger` 和 `NudgeService`。通过 `tokio::spawn` 在后台执行 review，不阻塞主对话。

**Tech Stack:** Rust (tokio async, serde, tracing)

---

## File Structure

### New Files
- `crates/hermes-core/src/nudge.rs` — Nudge module (types + service)

### Modified Files
- `crates/hermes-core/src/agent.rs:55-92` — Add `nudge_service` and `nudge_state` fields to `Agent`, update `new()` signature, integrate into `run_conversation`
- `crates/hermes-core/src/config.rs:96-134` — Add `NudgeConfig` to `Config` struct and merge/logic
- `crates/hermes-core/src/lib.rs:42-56` — Add `pub mod nudge` and re-export types

---

## Task 1: Create nudge.rs Module with Types

**Files:**
- Create: `crates/hermes-core/src/nudge.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/hermes-core/src/nudge.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nudge_trigger_enum() {
        assert_eq!(NudgeTrigger::None, NudgeTrigger::None);
        assert_eq!(NudgeTrigger::Memory, NudgeTrigger::Memory);
        assert_eq!(NudgeTrigger::Skill, NudgeTrigger::Skill);
        assert_eq!(NudgeTrigger::Both, NudgeTrigger::Both);
    }

    #[test]
    fn test_nudge_config_default() {
        let config = NudgeConfig::default();
        assert_eq!(config.memory_interval, 10);
        assert_eq!(config.skill_interval, 15);
    }

    #[test]
    fn test_nudge_config_disabled() {
        let config = NudgeConfig::disabled();
        assert_eq!(config.memory_interval, 0);
        assert_eq!(config.skill_interval, 0);
    }

    #[test]
    fn test_nudge_state_default() {
        let state = NudgeState::default();
        assert_eq!(state.turns_since_memory, 0);
        assert_eq!(state.iters_since_skill, 0);
    }

    #[test]
    fn test_nudge_service_check_triggers_memory() {
        let config = NudgeConfig { memory_interval: 2, skill_interval: 0 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // First turn - should not trigger
        let trigger = service.check_triggers(&mut state, 1, 0);
        assert_eq!(trigger, NudgeTrigger::None);

        // Second turn - should trigger memory
        let trigger = service.check_triggers(&mut state, 1, 0);
        assert_eq!(trigger, NudgeTrigger::Memory);
    }

    #[test]
    fn test_nudge_service_check_triggers_skill() {
        let config = NudgeConfig { memory_interval: 0, skill_interval: 3 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // 2 tool calls - should not trigger
        let trigger = service.check_triggers(&mut state, 1, 2);
        assert_eq!(trigger, NudgeTrigger::None);

        // 1 more tool call (total 3) - should trigger skill
        let trigger = service.check_triggers(&mut state, 1, 1);
        assert_eq!(trigger, NudgeTrigger::Skill);
    }

    #[test]
    fn test_nudge_service_check_triggers_both() {
        let config = NudgeConfig { memory_interval: 1, skill_interval: 1 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // 1 turn + 1 tool call = both triggers
        let trigger = service.check_triggers(&mut state, 1, 1);
        assert_eq!(trigger, NudgeTrigger::Both);
    }

    #[test]
    fn test_nudge_service_disabled() {
        let config = NudgeConfig::disabled();
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // Even with many turns, should never trigger when disabled
        for _ in 0..100 {
            let trigger = service.check_triggers(&mut state, 1, 100);
            assert_eq!(trigger, NudgeTrigger::None);
        }
    }

    #[test]
    fn test_get_prompt() {
        let service = NudgeService::new(NudgeConfig::default());
        
        assert!(service.get_prompt(NudgeTrigger::Memory).contains("memory"));
        assert!(service.get_prompt(NudgeTrigger::Skill).contains("skill"));
        assert!(service.get_prompt(NudgeTrigger::Both).contains("Memory"));
        assert!(service.get_prompt(NudgeTrigger::Both).contains("Skills"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p hermes-core nudge::tests -- --nocapture 2>&1`
Expected: FAIL because `nudge` module doesn't exist yet

- [ ] **Step 3: Write minimal implementation**

```rust
//! Nudge System — Background memory/skill review for Hermes Agent
//!
//! ## Overview
//! Implements periodic background review of conversations to prompt the agent
//! to save important information to memory or create/update skills.
//!
//! ## Trigger Conditions
//! - **Memory Nudge**: Every N user turns (config.memory_interval)
//! - **Skill Nudge**: Every N tool-call iterations (config.skill_interval)
//!
//! ## Reference
//! Python implementation: `run_agent.py` in NousResearch/hermes-agent

use serde::{Deserialize, Serialize};

/// Nudge trigger type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeTrigger {
    None,
    Memory,
    Skill,
    Both,
}

/// Nudge system configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NudgeConfig {
    /// Memory review interval in user turns (0 = disabled)
    pub memory_interval: usize,
    /// Skill review interval in tool-call iterations (0 = disabled)
    pub skill_interval: usize,
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            memory_interval: 10,
            skill_interval: 15,
        }
    }
}

impl NudgeConfig {
    /// Create a disabled config (used by subagents to prevent nested nudges)
    pub fn disabled() -> Self {
        Self {
            memory_interval: 0,
            skill_interval: 0,
        }
    }
}

/// Nudge trigger state (turns/iterations since last review)
#[derive(Debug, Clone)]
pub struct NudgeState {
    pub turns_since_memory: usize,
    pub iters_since_skill: usize,
}

impl Default for NudgeState {
    fn default() -> Self {
        Self {
            turns_since_memory: 0,
            iters_since_skill: 0,
        }
    }
}

/// Review prompts for background review agent
pub struct ReviewPrompts;

impl ReviewPrompts {
    /// Prompt for memory review
    pub const MEMORY_REVIEW: &'static str = concat!(
        "Review the conversation above and consider saving to memory if appropriate.\n",
        "Focus on:\n",
        "1. Has the user revealed things about themselves — their persona, desires, ",
        "preferences, or personal details worth remembering?\n",
        "2. Has the user expressed expectations about how you should behave, ",
        "their work style, or ways they want you to operate?\n\n",
        "If something stands out, save it using the memory tool. ",
        "If nothing is worth saving, just say 'Nothing to save.' and stop."
    );

    /// Prompt for skill review
    pub const SKILL_REVIEW: &'static str = concat!(
        "Review the conversation above and consider saving or updating a skill if appropriate.\n\n",
        "Focus on: was a non-trivial approach used to complete a task that required trial ",
        "and error, or changing course due to experiential findings along the way, or did ",
        "the user expect or desire a different method or outcome?\n\n",
        "If a relevant skill already exists, update it with what you learned. ",
        "Otherwise, create a new skill if the approach is reusable.\n",
        "If nothing is worth saving, just say 'Nothing to save.' and stop."
    );

    /// Combined prompt for both memory and skill review
    pub const COMBINED_REVIEW: &'static str = concat!(
        "Review the conversation above and consider two things:\n\n",
        "**Memory**: Has the user revealed things about themselves — their persona, ",
        "desires, preferences, or personal details? Has the user expressed expectations ",
        "about how you should behave, their work style, or ways they want you to operate? ",
        "If so, save using the memory tool.\n\n",
        "**Skills**: Was a non-trivial approach used to complete a task that required trial ",
        "and error, or changing course due to experiential findings along the way? If a relevant skill ",
        "already exists, update it. Otherwise, create a new one if the approach is reusable.\n\n",
        "Only act if there's something genuinely worth saving. ",
        "If nothing stands out, just say 'Nothing to save.' and stop."
    );
}

/// Nudge service - checks triggers and spawns background reviews
#[derive(Debug, Clone)]
pub struct NudgeService {
    config: NudgeConfig,
}

impl NudgeService {
    pub fn new(config: NudgeConfig) -> Self {
        Self { config }
    }

    /// Check if nudge should be triggered
    pub fn check_triggers(
        &self,
        state: &mut NudgeState,
        user_turn_count: usize,
        tool_calls_this_turn: usize,
    ) -> NudgeTrigger {
        state.turns_since_memory += 1;
        state.iters_since_skill += tool_calls_this_turn;

        let memory_triggered = self.config.memory_interval > 0
            && state.turns_since_memory >= self.config.memory_interval;

        let skill_triggered = self.config.skill_interval > 0
            && state.iters_since_skill >= self.config.skill_interval;

        if memory_triggered {
            state.turns_since_memory = 0;
        }
        if skill_triggered {
            state.iters_since_skill = 0;
        }

        match (memory_triggered, skill_triggered) {
            (true, true) => NudgeTrigger::Both,
            (true, false) => NudgeTrigger::Memory,
            (false, true) => NudgeTrigger::Skill,
            (false, false) => NudgeTrigger::None,
        }
    }

    /// Get prompt for trigger type
    pub fn get_prompt(&self, trigger: NudgeTrigger) -> &'static str {
        match trigger {
            NudgeTrigger::Memory => ReviewPrompts::MEMORY_REVIEW,
            NudgeTrigger::Skill => ReviewPrompts::SKILL_REVIEW,
            NudgeTrigger::Both => ReviewPrompts::COMBINED_REVIEW,
            NudgeTrigger::None => unreachable!(),
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p hermes-core nudge::tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-core/src/nudge.rs
git commit -m "feat(nudge): add NudgeSystem types and trigger logic"
```

---

## Task 2: Add NudgeConfig to Config struct

**Files:**
- Modify: `crates/hermes-core/src/config.rs:96-134`

- [ ] **Step 1: Add NudgeConfig field to Config struct**

Add this field to the `Config` struct at line 98:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub credentials: HashMap<String, String>,
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub nudge: NudgeConfig,  // ADD THIS LINE
}
```

- [ ] **Step 2: Update Config::default() to include NudgeConfig**

At line 127, update:

```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: DefaultsConfig::default(),
            credentials: HashMap::new(),
            gateway: GatewayConfig::default(),
            nudge: NudgeConfig::default(),  // ADD THIS LINE
        }
    }
}
```

- [ ] **Step 3: Update Config::merge() to handle nudge**

Add after the gateway merge block (around line 185):

```rust
// NudgeConfig merge - only override if non-default
if other.nudge != NudgeConfig::default() {
    self.nudge = other.nudge;
}
```

- [ ] **Step 4: Update Config::load_from_env() for nudge env vars**

Add after line 222 (before the closing brace of `load_from_env`):

```rust
if let Ok(val) = std::env::var("HERMES_NUDGE_MEMORY_INTERVAL") {
    if let Ok(interval) = val.parse() {
        self.nudge.memory_interval = interval;
    }
}
if let Ok(val) = std::env::var("HERMES_NUDGE_SKILL_INTERVAL") {
    if let Ok(interval) = val.parse() {
        self.nudge.skill_interval = interval;
    }
}
```

- [ ] **Step 5: Update Config::get() for nudge keys**

Add to the match statement in `get()` method (around line 249):

```rust
_ => {
    if let Some(val) = self.nudge.get(key) {
        return Some(val);
    }
    None
}
```

- [ ] **Step 6: Add get() method to NudgeConfig**

Add this impl block to `NudgeConfig` in the nudge.rs file (or we can put it in config.rs). For simplicity, add this method:

```rust
impl NudgeConfig {
    /// Get a config value by key
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "memory_interval" => Some(self.memory_interval.to_string()),
            "skill_interval" => Some(self.skill_interval.to_string()),
            _ => None,
        }
    }
}
```

- [ ] **Step 7: Run tests to verify compilation**

Run: `cargo build -p hermes-core 2>&1`
Expected: Compiles without errors

- [ ] **Step 8: Commit**

```bash
git add crates/hermes-core/src/config.rs crates/hermes-core/src/nudge.rs
git commit -m "feat(config): add NudgeConfig to Config struct"
```

---

## Task 3: Export NudgeModule in lib.rs

**Files:**
- Modify: `crates/hermes-core/src/lib.rs:42-56`

- [ ] **Step 1: Add nudge module declaration**

Add after line 55 (after `pub mod delegate;`):

```rust
pub mod nudge;
```

- [ ] **Step 2: Re-export nudge types**

Add after line 70 (after `pub use gateway::*;`):

```rust
pub use nudge::{NudgeConfig, NudgeService, NudgeState, NudgeTrigger};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p hermes-core 2>&1`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/src/lib.rs
git commit -m "feat(nudge): export NudgeModule in lib.rs"
```

---

## Task 4: Integrate NudgeService into Agent

**Files:**
- Modify: `crates/hermes-core/src/agent.rs:55-92`

- [ ] **Step 1: Add nudge fields to Agent struct**

At line 55-61, update the `Agent` struct:

```rust
pub struct Agent {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<dyn ToolDispatcher>,
    session_store: Arc<dyn SessionStore>,
    config: AgentConfig,
    // NEW: nudge fields
    nudge_service: Arc<NudgeService>,
    nudge_state: NudgeState,
}
```

- [ ] **Step 2: Update Agent::new() to accept NudgeConfig**

At line 79-91, update the signature and body:

```rust
pub fn new(
    provider: Arc<dyn LlmProvider>,
    tools: Arc<dyn ToolDispatcher>,
    session_store: Arc<dyn SessionStore>,
    config: AgentConfig,
    nudge_config: NudgeConfig,  // ADD THIS PARAM
) -> Self {
    Self {
        provider,
        tools,
        session_store,
        config,
        // NEW: initialize nudge
        nudge_service: Arc::new(NudgeService::new(nudge_config)),
        nudge_state: NudgeState::default(),
    }
}
```

- [ ] **Step 3: Add helper method for disabled nudge config**

Add after the `new()` method:

```rust
impl Agent {
    /// Create Agent with nudge disabled (for subagents)
    pub fn new_with_nudge_disabled(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
    ) -> Self {
        Self::new(
            provider,
            tools,
            session_store,
            config,
            NudgeConfig::disabled(),
        )
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build -p hermes-core 2>&1`
Expected: Compiles without errors (we'll integrate into run_conversation later)

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-core/src/agent.rs
git commit -m "feat(agent): add nudge fields and new_with_nudge_disabled constructor"
```

---

## Task 5: Integrate NudgeService into run_conversation

**Files:**
- Modify: `crates/hermes-core/src/agent.rs:93-223`

- [ ] **Step 1: Find the FinishReason::Stop handling**

Look at lines 150-172 in `agent.rs`. We need to add nudge trigger check before the final response.

At line 150, inside `FinishReason::Stop` match arm, add after handling tool_calls (around line 175, before `iterations += 1; continue;`):

```rust
// Track tool calls for skill nudge
self.nudge_state.iters_since_skill += tool_calls.len();
```

Then after the tool_calls handling block (around line 176, before `return Ok(...)`), add the nudge trigger check:

```rust
// ========== Nudge: Check triggers ==========
let trigger = self.nudge_service.check_triggers(
    &mut self.nudge_state,
    messages.len(),
    0,  // no tool calls this turn
);

if trigger != NudgeTrigger::None {
    let prompt = self.nudge_service.get_prompt(trigger);
    let messages_clone = messages.clone();
    
    // Spawn background review (fire-and-forget)
    let provider = self.provider.clone();
    let tools = self.tools.clone();
    let session_store = self.session_store.clone();
    
    tokio::spawn(async move {
        let review_agent = Agent::new_with_nudge_disabled(
            provider,
            tools,
            session_store,
            AgentConfig {
                max_iterations: 8,
                ..Default::default()
            },
        );
        
        if let Err(e) = review_agent.run_conversation(ConversationRequest {
            content: prompt.to_string(),
            session_id: None,
            system_prompt: None,
        }).await {
            tracing::debug!("Background review failed: {}", e);
        }
    });
}
```

- [ ] **Step 2: Add tokio::spawn import if not present**

Check imports at top of agent.rs. If `tokio::spawn` is not available, we need to add it. Since the project uses `#[tokio::test]`, tokio should already be available.

- [ ] **Step 3: Verify compilation**

Run: `cargo build -p hermes-core 2>&1`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/src/agent.rs
git commit -m "feat(agent): integrate NudgeService into run_conversation"
```

---

## Task 6: Update Agent Construction in hermes-cli

**Files:**
- Modify: `crates/hermes-cli/src/chat.rs` or wherever Agent is constructed

- [ ] **Step 1: Find where Agent::new() is called**

Run: `grep -r "Agent::new" crates/ --include="*.rs"`

- [ ] **Step 2: Update the Agent::new() call to pass NudgeConfig**

Most likely in `hermes-cli/src/chat.rs` or similar. Add the nudge_config parameter:

```rust
let agent = Agent::new(
    provider,
    tools,
    session_store,
    agent_config,
    nudge_config,  // ADD THIS - from config.nudge
);
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --all 2>&1`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-cli/src/chat.rs  # or whatever file was modified
git commit -m "feat(cli): pass NudgeConfig to Agent::new()"
```

---

## Task 7: Write Integration Test

**Files:**
- Create: `crates/hermes-core/src/tests/nudge_tests.rs`

- [ ] **Step 1: Write integration test**

```rust
//! Integration tests for Nudge system

#[cfg(test)]
mod nudge_integration {
    use crate::{NudgeConfig, NudgeService, NudgeState, NudgeTrigger};

    #[test]
    fn test_memory_nudge_triggers_at_interval() {
        let config = NudgeConfig { memory_interval: 3, skill_interval: 0 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // Turns 1 and 2 - no trigger
        assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::None);
        assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::None);

        // Turn 3 - should trigger
        assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::Memory);
    }

    #[test]
    fn test_skill_nudge_triggers_at_interval() {
        let config = NudgeConfig { memory_interval: 0, skill_interval: 5 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // 4 tool calls - no trigger
        assert_eq!(service.check_triggers(&mut state, 1, 4), NudgeTrigger::None);

        // 1 more (total 5) - should trigger
        assert_eq!(service.check_triggers(&mut state, 1, 1), NudgeTrigger::Skill);
    }

    #[test]
    fn test_both_nudges_trigger_simultaneously() {
        let config = NudgeConfig { memory_interval: 2, skill_interval: 3 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // Turn 2 + 3 tool calls = both
        state.turns_since_memory = 1;  // Simulate 1 turn already
        assert_eq!(service.check_triggers(&mut state, 1, 3), NudgeTrigger::Both);
    }

    #[test]
    fn test_disabled_nudge_never_triggers() {
        let config = NudgeConfig::disabled();
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        for _ in 0..1000 {
            assert_eq!(
                service.check_triggers(&mut state, 1, 1000),
                NudgeTrigger::None
            );
        }
    }

    #[test]
    fn test_nudge_state_resets_after_trigger() {
        let config = NudgeConfig { memory_interval: 2, skill_interval: 2 };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // Trigger both
        assert_eq!(service.check_triggers(&mut state, 1, 2), NudgeTrigger::Both);

        // Next turn should not trigger (counters reset)
        assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::None);
    }
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test -p hermes-core nudge_integration -- --nocapture`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/tests/nudge_tests.rs
git commit -m "test(nudge): add Nudge system integration tests"
```

---

## Self-Review Checklist

1. **Spec coverage**: All requirements from design doc are implemented?
   - [x] NudgeConfig with memory_interval and skill_interval
   - [x] NudgeState tracking turns and iterations
   - [x] NudgeTrigger enum (None, Memory, Skill, Both)
   - [x] ReviewPrompts with all three prompts
   - [x] NudgeService::check_triggers() logic
   - [x] NudgeService::get_prompt()
   - [x] Config integration
   - [x] Agent integration

2. **Placeholder scan**: No "TBD", "TODO", or placeholder code remaining

3. **Type consistency**: All types match across tasks?
   - NudgeConfig fields: memory_interval, skill_interval ✓
   - NudgeState fields: turns_since_memory, iters_since_skill ✓
   - NudgeTrigger enum variants: None, Memory, Skill, Both ✓

---

**Plan complete.** Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
