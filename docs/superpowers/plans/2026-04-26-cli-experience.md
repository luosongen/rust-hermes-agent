# CLI 体验改进实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 整合 UI 组件到 REPL，实现加载动画和命令补全，提供异步行编辑体验

**Architecture:** 保持 tokio 异步 I/O 框架，用 rustyline + spawn_blocking 处理输入（异步 readline 库不稳定），连接 CommandHistory 和 LoadingAnimation 到 REPL

**Tech Stack:** tokio, rustyline (with spawn_blocking), ANSI escape codes

---

## 文件结构

```
crates/hermes-cli/
├── src/
│   ├── chat.rs                    # 修改: 整合所有组件
│   └── ui/
│       ├── mod.rs                 # 修改: 导出 LoadingAnimation
│       ├── streaming_output.rs    # 修改: 实现 start_loading/stop_loading
│       ├── completer.rs          # 修改: 实现 complete_args
│       └── line_reader.rs         # 新增: 异步 readline 封装
└── Cargo.toml                    # 修改: 添加 rustyline 依赖
```

---

## Task 1: 添加依赖

**Files:**
- Modify: `crates/hermes-cli/Cargo.toml`

- [ ] **Step 1: 添加 rustyline 依赖**

```toml
[dependencies]
# CLI UI
rustyline = "15"  # 行编辑和历史
```

---

## Task 2: 实现 LoadingAnimation

**Files:**
- Modify: `crates/hermes-cli/src/ui/streaming_output.rs`

- [ ] **Step 1: 实现 start_loading**

```rust
/// 显示加载动画
pub fn start_loading(&self, message: &str) {
    if !self.enabled.load(Ordering::SeqCst) {
        self.enabled.store(true, Ordering::SeqCst);
        let enabled = self.enabled.clone();
        let msg = message.to_string();
        tokio::spawn(async move {
            let spinner = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
            let mut i = 0;
            while enabled.load(Ordering::SeqCst) {
                print!("\r{} {}{}\x1B[K", spinner.chars().nth(i % 10).unwrap_or(' '), msg, "...");
                std::io::stdout().flush().ok();
                tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                i += 1;
            }
            print!("\r\x1B[K");
            std::io::stdout().flush().ok();
        });
    }
}
```

- [ ] **Step 2: 实现 stop_loading**

```rust
/// 停止加载动画
pub fn stop_loading(&self) {
    self.enabled.store(false, Ordering::SeqCst);
}
```

- [ ] **Step 3: 添加 enabled 字段到 StreamingOutput**

在 `StreamingOutput` 结构体中添加:
```rust
enabled: Arc<AtomicBool>,
```

- [ ] **Step 4: 运行测试验证**

```bash
cargo test -p hermes-cli
```

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-cli/src/ui/streaming_output.rs
git commit -m "feat(cli): 实现 start_loading 和 stop_loading 动画"
```

---

## Task 3: 实现 CommandCompleter.complete_args

**Files:**
- Modify: `crates/hermes-cli/src/ui/completer.rs`

- [ ] **Step 1: 实现 complete_args 方法**

```rust
/// 补全命令参数
pub fn complete_args(&self, command: &str, _partial: &str) -> Vec<String> {
    match command.trim_start_matches('/').split_whitespace().next().unwrap_or("") {
        "model" => vec![
            "openai/gpt-4o".to_string(),
            "openai/gpt-4o-mini".to_string(),
            "anthropic/claude-3-5-sonnet-20241022".to_string(),
        ],
        "context" => vec!["compress".to_string(), "clear".to_string(), "status".to_string()],
        "tokens" => vec!["status".to_string()],
        "system" => vec!["prompt".to_string(), "role".to_string()],
        _ => vec![],
    }
}
```

- [ ] **Step 2: 运行测试验证**

```bash
cargo test -p hermes-cli
```

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-cli/src/ui/completer.rs
git commit -m "feat(cli): 实现 complete_args 命令参数补全"
```

---

## Task 4: 创建 LineReader 封装

**Files:**
- Create: `crates/hermes-cli/src/ui/line_reader.rs`

- [ ] **Step 1: 创建 LineReader 结构体**

```rust
//! 异步 readline 封装

use rustyline::{config::Config, Editor};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct LineReader {
    editor: Arc<Mutex<Editor<()>>>,
}

impl LineReader {
    pub fn new(history_file: Option<&str>) -> Self {
        let config = Config::builder()
            .history_ignore_dups(true)
            .build();
        let mut editor = Editor::with_config(config);
        if let Some(path) = history_file {
            let _ = editor.load_history(path);
        }
        Self {
            editor: Arc::new(Mutex::new(editor)),
        }
    }

    pub async fn read_line(&self, prompt: &str) -> Result<String, std::io::Error> {
        let editor = self.editor.clone();
        let prompt = prompt.to_string();
        
        // 在阻塞线程中运行 rustyline（因为它需要同步 IO）
        tokio::task::spawn_blocking(move || {
            let mut editor = editor.blocking_lock();
            editor.readline(&prompt)
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}
```

- [ ] **Step 2: 更新 ui/mod.rs 导出**

```rust
pub mod line_reader;
pub use line_reader::LineReader;
```

- [ ] **Step 3: 运行 cargo check 验证编译**

```bash
cargo check -p hermes-cli
```

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-cli/src/ui/line_reader.rs crates/hermes-cli/src/ui/mod.rs
git commit -m "feat(cli): 添加异步 LineReader 封装"
```

---

## Task 5: 整合到 REPL 主循环

**Files:**
- Modify: `crates/hermes-cli/src/chat.rs`

- [ ] **Step 1: 添加导入**

```rust
use crate::ui::{
    CliDisplay, LineReader, LoadingAnimation, SlashCommandCompleter,
    StreamingOutput,
};
```

- [ ] **Step 2: 修改 REPL 循环**

替换原始的 `BufReader` 循环:

```rust
// 创建 UI 组件
let streaming_output = StreamingOutput::new();
let loading_animation = LoadingAnimation::new();
let completer = SlashCommandCompleter::new();
let mut line_reader = LineReader::new(Some("hermes_history.txt"));

loop {
    let line = match line_reader.read_line("> ").await {
        Ok(l) => l,
        Err(_) => break,
    };
    
    let line = line.trim();
    if line.is_empty() {
        continue;
    }
    
    // 显示加载动画
    loading_animation.start("处理中");
    
    // 调用 Agent
    let response = agent.write().await.run_conversation(...).await;
    
    // 停止加载动画
    loading_animation.stop();
    
    match response {
        Ok(resp) => println!("[Agent] {}\n", resp.content),
        Err(e) => eprintln!("[错误] {}\n", e),
    }
}
```

- [ ] **Step 3: 运行 cargo check 验证编译**

```bash
cargo check -p hermes-cli
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p hermes-cli
```

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-cli/src/chat.rs
git commit -m "feat(cli): 整合 LineReader 和 LoadingAnimation 到 REPL"
```

---

## Task 6: 端到端测试

- [ ] **Step 1: 手动测试 REPL 功能**

```bash
cargo run --bin hermes-cli -- chat
```

验证:
- [ ] 输入 "test" 后按回车，显示加载动画
- [ ] Agent 返回后，动画消失
- [ ] 按上箭头调出历史命令
- [ ] 输入 `/model ` 后按 Tab，显示模型补全

- [ ] **Step 2: 提交最终变更**

```bash
git add -A
git commit -m "feat(cli): 完成 CLI 体验改进 - 加载动画、命令补全、行编辑"
```

---

## 成功标准检查清单

- [ ] `start_loading()` 显示旋转动画
- [ ] `stop_loading()` 清除动画
- [ ] `complete_args("/model", "")` 返回模型列表
- [ ] `LineReader::read_line()` 支持行编辑
- [ ] 上箭头调出历史命令
- [ ] Tab 触发命令补全
- [ ] REPL 循环正常工作
- [ ] 现有测试全部通过
