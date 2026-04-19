# Agent Nudge System 设计文档

**日期**: 2026-04-19
**状态**: 设计阶段

---

## 1. 概述

本文档描述了 Rust 版 Hermes Agent 的 Nudge System 实现方案，完整复刻 Python 版的功能。

### 1.1 核心功能

- **Memory Nudge**: 每 N 轮对话后，spawn background review agent 询问是否保存用户信息（偏好、习惯、工作方式）
- **Skill Nudge**: 复杂任务（多次工具调用）后，spawn review agent 询问是否创建/更新技能
- **Background Review**: 后台运行，不阻塞主对话

### 1.2 参考实现

Python 版 Hermes Agent 的 Nudge System 实现位于 `run_agent.py`：
- `_memory_nudge_interval` / `_skill_nudge_interval` 配置
- `_spawn_background_review()` 方法
- `_MEMORY_REVIEW_PROMPT` / `_SKILL_REVIEW_PROMPT` / `_COMBINED_REVIEW_PROMPT` 提示词

---

## 2. 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                         AIAgent (主循环)                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │ NudgeConfig │  │NudgeState  │  │  ReviewPromptBuilder   │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│         │                │                      │              │
│         └────────────────┼──────────────────────┘              │
│                          ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              NudgeService (触发器)                       │   │
│  │  - memory_nudge_interval (默认10轮)                      │   │
│  │  - skill_nudge_interval (默认15次工具调用)               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                          │                                     │
│                          ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │         spawn_background_review()                         │   │
│  │  - 创建 SubAgent fork (共享 memory_store)                │   │
│  │  - 传入 conversation_history + review_prompt              │   │
│  │  - 后台线程执行 (tokio::spawn)                          │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SubAgent (后台 Review)                         │
│  - 使用相同 model/tools                                         │
│  - max_iterations = 8 (轻量)                                   │
│  - nudge_service = disabled (禁用嵌套)                          │
│  - 调用 memory_tool / skill_*_tool 写入                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. 核心组件

### 3.1 NudgeConfig

```rust
/// Nudge 系统配置
#[derive(Debug, Clone)]
pub struct NudgeConfig {
    /// 记忆提醒间隔（用户轮次），0 = 禁用
    pub memory_interval: usize,
    /// 技能创建提醒间隔（工具调用次数），0 = 禁用
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
```

### 3.2 NudgeState

```rust
/// Nudge 触发状态
#[derive(Debug, Clone)]
pub struct NudgeState {
    /// 距上次记忆提醒以来的轮次
    pub turns_since_memory: usize,
    /// 距上次技能提醒以来的工具调用次数
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
```

### 3.3 NudgeTrigger

```rust
/// Nudge 触发类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeTrigger {
    None,
    Memory,
    Skill,
    Both,
}
```

### 3.4 ReviewPrompts

```rust
pub struct ReviewPrompts;

impl ReviewPrompts {
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

    pub const SKILL_REVIEW: &'static str = concat!(
        "Review the conversation above and consider saving or updating a skill if appropriate.\n\n",
        "Focus on: was a non-trivial approach used to complete a task that required trial ",
        "and error, or changing course due to experiential findings along the way, or did ",
        "the user expect or desire a different method or outcome?\n\n",
        "If a relevant skill already exists, update it with what you learned. ",
        "Otherwise, create a new skill if the approach is reusable.\n",
        "If nothing is worth saving, just say 'Nothing to save.' and stop."
    );

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
```

---

## 4. NudgeService 实现

### 4.1 服务结构

```rust
/// Nudge 服务
pub struct NudgeService {
    config: NudgeConfig,
    prompts: ReviewPrompts,
}

impl NudgeService {
    pub fn new(config: NudgeConfig) -> Self {
        Self {
            config,
            prompts: ReviewPrompts,
        }
    }

    /// 创建禁用的 NudgeService（用于 subagent）
    pub fn disabled() -> Self {
        Self {
            config: NudgeConfig {
                memory_interval: 0,
                skill_interval: 0,
            },
            prompts: ReviewPrompts,
        }
    }
}
```

### 4.2 触发检查

```rust
impl NudgeService {
    /// 检查是否应该触发 nudge
    pub fn check_triggers(
        &self,
        state: &mut NudgeState,
        user_turn_count: usize,
        tool_calls_this_turn: usize,
    ) -> NudgeTrigger {
        // 更新状态
        state.turns_since_memory += 1;
        state.iters_since_skill += tool_calls_this_turn;

        let memory_triggered = self.config.memory_interval > 0
            && state.turns_since_memory >= self.config.memory_interval;

        let skill_triggered = self.config.skill_interval > 0
            && state.iters_since_skill >= self.config.skill_interval;

        // 重置计数器
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

    /// 获取对应触发类型的提示词
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

### 4.3 Spawn Background Review

```rust
impl NudgeService {
    /// Spawn 后台 review agent
    pub async fn spawn_review(
        &self,
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        messages: Vec<Message>,
        trigger: NudgeTrigger,
    ) {
        let prompt = self.get_prompt(trigger);

        // 后台执行，不阻塞
        tokio::spawn(async move {
            let review_agent = Agent::new(
                provider,
                tools,
                session_store,
                AgentConfig {
                    max_iterations: 8,  // 轻量
                    model: "openai/gpt-4o".to_string(),
                    ..Default::default()
                },
                NudgeConfig::disabled(),  // 禁用嵌套 nudge
            );

            let request = ConversationRequest {
                content: prompt.to_string(),
                session_id: None,  // 新会话，不关联
                system_prompt: None,
            };

            if let Err(e) = review_agent.run_conversation(request).await {
                tracing::debug!("Background review failed: {}", e);
            }
        });
    }
}
```

---

## 5. Agent 集成

### 5.1 扩展 Agent 结构

```rust
pub struct Agent {
    provider: Arc<dyn LlmProvider>,
    tools: Arc<dyn ToolDispatcher>,
    session_store: Arc<dyn SessionStore>,
    config: AgentConfig,
    // 新增
    nudge_service: Arc<NudgeService>,
    nudge_state: NudgeState,
}

impl Agent {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tools: Arc<dyn ToolDispatcher>,
        session_store: Arc<dyn SessionStore>,
        config: AgentConfig,
        nudge_config: NudgeConfig,
    ) -> Self {
        Self {
            provider,
            tools,
            session_store,
            config,
            nudge_service: Arc::new(NudgeService::new(nudge_config)),
            nudge_state: NudgeState::default(),
        }
    }
}
```

### 5.2 集成到 run_conversation

```rust
impl Agent {
    pub async fn run_conversation(
        &self,
        request: ConversationRequest,
    ) -> Result<ConversationResponse, AgentError> {
        let messages = /* 加载历史或新建 */;
        let mut iterations = 0;

        loop {
            // ... 现有 LLM 调用逻辑 ...

            let response = self.provider.chat(chat_request).await?;

            match response.finish_reason {
                FinishReason::Stop => {
                    if let Some(tool_calls) = response.tool_calls {
                        let tool_count = tool_calls.len();
                        for call in &tool_calls {
                            let result = self.tools.dispatch(call, context.clone()).await?;
                            messages.push(Message::tool_result(call.id.clone(), result));
                        }
                        iterations += 1;

                        // ========== Nudge: 更新工具调用计数 ==========
                        self.nudge_state.iters_since_skill += tool_count;

                        continue;
                    }

                    // ========== Nudge: 检查触发 ==========
                    let trigger = self.nudge_service.check_triggers(
                        &mut self.nudge_state,
                        messages.len(),
                        0,  // 这轮没有工具调用
                    );

                    if trigger != NudgeTrigger::None {
                        self.nudge_service.spawn_review(
                            self.provider.clone(),
                            self.tools.clone(),
                            self.session_store.clone(),
                            messages.clone(),
                            trigger,
                        );
                    }

                    // 保存到会话存储并返回
                    // ...
                }
                // ...
            }
        }
    }
}
```

---

## 6. 配置扩展

### 6.1 Config 结构

```toml
# ~/.config/hermes-agent/config.toml

[nudge]
memory_interval = 10
skill_interval = 15
```

### 6.2 Config 解析

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub nudge: NudgeConfig,
    // ...
}

#[derive(Debug, Clone, Deserialize)]
pub struct NudgeConfig {
    #[serde(default = "default_memory_interval")]
    pub memory_interval: usize,
    #[serde(default = "default_skill_interval")]
    pub skill_interval: usize,
}

fn default_memory_interval() -> usize { 10 }
fn default_skill_interval() -> usize { 15 }
```

---

## 7. 文件结构

### 新增文件

| 文件路径 | 说明 |
|---------|------|
| `crates/hermes-core/src/nudge.rs` | Nudge 模块主文件 |

### 修改文件

| 文件路径 | 修改内容 |
|---------|---------|
| `crates/hermes-core/src/agent.rs` | 集成 NudgeService |
| `crates/hermes-core/src/config.rs` | 添加 NudgeConfig 解析 |
| `crates/hermes-core/src/lib.rs` | 导出 nudge 模块 |

---

## 8. 测试计划

1. **单元测试**: `NudgeService::check_triggers` 触发逻辑
2. **集成测试**: Nudge 配置加载和解析
3. **手动测试**: 实际对话触发 review

---

## 9. 后续扩展

- **Callback 机制**: Review 完成后通知 CLI 显示摘要
- **Result 解析**: 解析 review agent 的工具调用结果并打印
- **Skill Nudge 增强**: 支持更复杂的技能创建逻辑
