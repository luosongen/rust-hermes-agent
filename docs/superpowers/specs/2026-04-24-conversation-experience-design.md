# Conversation Experience Features Design

> **For agentic workers:** Implementation using superpowers:subagent-driven-development

**Goal:** 实现对话体验增强模块：Display Handler、Title Generator、Trajectory Saver

**Architecture:** DisplayHandler trait 放入 hermes-core，CLI 实现放入 hermes-cli；Title Generator 和 Trajectory Saver 放入 hermes-core

**Tech Stack:** Rust, tokio, async-trait

---

## 1. Overview

三个互补的对话体验模块：

1. **Display Handler** — 抽象 trait + CLI 实现，提供工具执行进度、spinner、diff 格式化、thinking 预览
2. **Title Generator** — 首条对话完成后异步生成会话标题，存入 SessionStore metadata
3. **Trajectory Saver** — 会话结束时自动保存对话轨迹到 JSONL

### 1.1 Python Reference

- `agent/display.py` (1037 lines)
- `agent/title_generator.py` (125 lines)
- `agent/trajectory.py` (56 lines)

### 1.2 Rust Current State

- `hermes-core/src/agent.rs` — Agent 主循环，无显示反馈
- `hermes-cli/src/chat.rs` — 基础 `println!("[Agent] {}")` 输出
- `hermes-core/src/config/display.rs` — `DisplayConfig` 配置结构（compact, tool_progress, skin）
- `hermes-memory/src/session.rs` — `SessionMetadata` 已有基础字段
- `hermes-auxiliary` crate — 已存在，可用于 title generation 的 LLM 调用

缺失：
- DisplayHandler trait 及 CLI 实现
- TitleGenerator 服务
- TrajectorySaver
- SessionMetadata 的 title/message_count 字段

---

## 2. Module Architecture

```
hermes-core/src/
├── display.rs              # DisplayHandler trait
├── title_generator.rs      # TitleGenerator
├── trajectory.rs           # TrajectorySaver
└── lib.rs                  # 修改：export 新模块

hermes-cli/src/
└── display.rs              # CliDisplay 实现

hermes-memory/src/
└── session.rs              # 修改：SessionMetadata 扩展
```

---

## 3. Display Handler

### 3.1 Trait Design

```rust
/// 显示处理 trait — Agent 工具执行和思考的显示反馈
#[async_trait]
pub trait DisplayHandler: Send + Sync {
    /// 工具开始执行
    fn tool_started(&self, tool_name: &str, args: &serde_json::Value);

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
```

### 3.2 No-op 实现

```rust
/// 默认无操作实现（当没有 display handler 注册时使用）
pub struct NoopDisplay;

impl DisplayHandler for NoopDisplay {
    fn tool_started(&self, _tool_name: &str, _args: &serde_json::Value) {}
    fn tool_completed(&self, _tool_name: &str, _result: &str) {}
    fn tool_failed(&self, _tool_name: &str, _error: &str) {}
    fn thinking_chunk(&self, _chunk: &str) {}
    fn show_diff(&self, _filename: &str, _old: &str, _new: &str) {}
    fn spinner_start(&self, _message: &str) {}
    fn spinner_stop(&self) {}
    fn flush(&self) {}
}
```

### 3.3 Agent 集成

`Agent` 结构体新增字段：

```rust
pub struct Agent {
    // ... existing fields ...
    display_handler: Option<Arc<dyn DisplayHandler>>,
}
```

Agent::new() 新增参数：

```rust
pub fn new(
    // ... existing params ...
    display_handler: Option<Arc<dyn DisplayHandler>>,
) -> Self
```

在工具执行前后调用 display handler。

---

## 4. Title Generator

### 4.1 TitleGenerator

```rust
/// 会话标题生成器
pub struct TitleGenerator {
    client: Arc<dyn LlmProvider>,
}

impl TitleGenerator {
    pub fn new(client: Arc<dyn LlmProvider>) -> Self {
        Self { client }
    }

    /// 生成标题（异步）
    pub async fn generate(
        &self,
        user_message: &str,
        assistant_response: &str,
    ) -> Option<String> {
        // 截断长消息
        let user_snippet = &user_message[..user_message.len().min(500)];
        let assistant_snippet = &assistant_response[..assistant_response.len().min(500)];

        let request = ChatRequest {
            model: ModelId::new("openai", "gpt-4o-mini"), // 使用便宜模型
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

        match self.client.chat(request).await {
            Ok(response) => {
                let title = response.content.trim().to_string();
                if title.is_empty() { None } else { Some(title) }
            }
            Err(_) => None,
        }
    }
}
```

### 4.2 触发逻辑

Agent 在 `run_conversation` 中判断：
- 如果当前会话消息数为 2（1 user + 1 assistant），且 title 为空
- 后台 spawn 一个 task 生成标题
- 不阻塞主响应

```rust
// 在 Agent::run_conversation 中
if messages.len() == 2 && session_title.is_none() {
    if let Some(generator) = &self.title_generator {
        let generator = generator.clone();
        let user_msg = user_content.to_string();
        let assistant_msg = response.content.clone();
        let store = self.session_store.clone();
        let sid = session_id.clone();
        tokio::spawn(async move {
            if let Some(title) = generator.generate(&user_msg, &assistant_msg).await {
                let _ = store.update_title(&sid, &title).await;
            }
        });
    }
}
```

---

## 5. Trajectory Saver

### 5.1 TrajectorySaver

```rust
/// 轨迹保存器
pub struct TrajectorySaver {
    output_dir: PathBuf,
}

impl TrajectorySaver {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self { output_dir: output_dir.into() }
    }

    /// 保存轨迹
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
            timestamp: SystemTime::now(),
            messages: messages.iter().map(|m| m.into()).collect(),
        };

        let line = serde_json::to_string(&entry)?;
        let path = self.output_dir.join(filename);

        // 追加到 JSONL
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
}

/// 轨迹条目（ShareGPT 格式）
#[derive(Serialize)]
struct TrajectoryEntry {
    model: String,
    completed: bool,
    timestamp: SystemTime,
    messages: Vec<TrajectoryMessage>,
}

#[derive(Serialize)]
struct TrajectoryMessage {
    role: String,
    content: String,
}
```

### 5.2 触发逻辑

Agent 在 `run_conversation` 返回前调用：

```rust
// 在 Agent::run_conversation 返回前
if let Some(saver) = &self.trajectory_saver {
    let completed = result.is_ok();
    let _ = saver.save(&messages, &self.config.model, completed);
}
```

---

## 6. SessionMetadata 扩展

```rust
/// 会话元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub title: Option<String>,
    pub message_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub model: Option<String>,
    pub source: String,
}
```

新增 SessionStore 方法：

```rust
async fn update_title(&self, session_id: &str, title: &str) -> Result<(), SessionError>;
async fn get_metadata(&self, session_id: &str) -> Result<SessionMetadata, SessionError>;
```

---

## 7. CLI Display Implementation

```rust
/// CLI 显示实现 — ANSI spinner、diff 颜色、工具预览
pub struct CliDisplay {
    config: DisplayConfig,
}

impl DisplayHandler for CliDisplay {
    fn tool_started(&self, tool_name: &str, args: &serde_json::Value) {
        if self.config.tool_progress {
            eprint!("\r{} {}...", spinner_frame(), tool_name);
        }
    }

    fn tool_completed(&self, tool_name: &str, result: &str) {
        eprintln!("\r{} {} done", "✓".green(), tool_name);
    }

    fn tool_failed(&self, tool_name: &str, error: &str) {
        eprintln!("\r{} {} failed: {}", "✗".red(), tool_name, error);
    }

    fn show_diff(&self, filename: &str, old: &str, new: &str) {
        // 使用 similar 或 diff 库格式化统一 diff
    }

    fn thinking_chunk(&self, chunk: &str) {
        // 流式打印 thinking 内容
    }

    fn spinner_start(&self, message: &str) {
        eprint!("\r{} {}", spinner_frame(), message);
    }

    fn spinner_stop(&self) {
        eprint!("\r\x1b[K"); // 清除行
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }
}
```

---

## 8. File Structure Summary

```
crates/hermes-core/src/
├── display.rs              # 新增 (~100 lines)
├── title_generator.rs      # 新增 (~80 lines)
├── trajectory.rs           # 新增 (~70 lines)
└── lib.rs                  # 修改：export 新模块

crates/hermes-cli/src/
└── display.rs              # 新增 (~120 lines)

crates/hermes-memory/src/
└── session.rs              # 修改：SessionMetadata 扩展
```

---

## 9. Dependencies

```toml
# hermes-core/Cargo.toml (新增)
[dependencies]
# 已有依赖足够

# hermes-cli/Cargo.toml (新增)
[dependencies]
crossterm = "0.28"  # ANSI 颜色和终端控制
similar = "2.7"     # diff 格式化
```

---

## 10. Implementation Phases

### Phase 1: DisplayHandler trait + NoopDisplay (P0)
- Create `hermes-core/src/display.rs`
- 定义 `DisplayHandler` trait
- 实现 `NoopDisplay`
- Agent 集成（可选参数）

### Phase 2: CliDisplay (P0)
- Create `hermes-cli/src/display.rs`
- 实现 `CliDisplay`（spinner、工具进度、diff）
- `chat.rs` 注入 CliDisplay

### Phase 3: Title Generator (P0)
- Create `hermes-core/src/title_generator.rs`
- 扩展 `SessionMetadata`
- Agent 集成（首条对话后异步触发）

### Phase 4: Trajectory Saver (P1)
- Create `hermes-core/src/trajectory.rs`
- Agent 集成（返回前保存）

### Phase 5: Integration Tests (P1)
- 测试 DisplayHandler trait 对象安全性
- 测试 TitleGenerator mock
- 测试 TrajectorySaver 文件写入

---

## 11. Testing Strategy

- **DisplayHandler:** Unit tests 验证 trait 对象安全性，NoopDisplay 所有方法无 panic
- **CliDisplay:** 集成测试验证 ANSI 输出格式
- **TitleGenerator:** Mock LlmProvider 测试标题生成流程
- **TrajectorySaver:** 临时目录测试 JSONL 写入和格式
