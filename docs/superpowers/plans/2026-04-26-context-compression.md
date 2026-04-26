# Context Compression 混合方案实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现上下文压缩质量改进 — 确定性元数据提取 + LLM 生成"原因"摘要混合方案

**Architecture:**
- 新增 `MetadataExtractor` 模块，从消息中确定性提取文件路径、符号引用、决策
- 新增 `SegmentSummary` 结构，包含 goal/progress/reasoning/remaining
- 重构 `ContextCompressor::compress()` 使用双轨方案
- 保持向后兼容，现有触发机制不变

**Tech Stack:** Rust, serde, regex, chrono, hermes-core

---

## 文件结构

```
crates/hermes-core/src/
├── context_compressor.rs    # 修改：集成 MetadataExtractor
├── metadata_extractor.rs   # 新增：确定性元数据提取
└── types.rs                # 修改：新增类型定义

crates/hermes-memory/src/
└── compressed.rs           # 修改：更新 CompressedSegment 结构
```

---

## Task 1: 添加新类型定义

**Files:**
- Modify: `crates/hermes-core/src/types.rs`

- [ ] **Step 1: 添加 FileAction 和 SymbolKind 枚举**

```rust
/// 文件操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAction {
    Read,
    Write,
    Created,
    Modified,
    Deleted,
}

/// 符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Trait,
    Impl,
    Enum,
    Module,
    Type,
    Constant,
}
```

- [ ] **Step 2: 添加 FileRef 结构体**

```rust
/// 单个文件引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub action: FileAction,
    pub snippet: Option<String>,
}
```

- [ ] **Step 3: 添加 SymbolRef 结构体**

```rust
/// 符号引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub line: Option<u32>,
}
```

- [ ] **Step 4: 添加 Decision 结构体**

```rust
/// 决策记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub description: String,
    pub chosen_option: String,
    pub alternatives: Vec<String>,
    pub rationale: String,
}
```

- [ ] **Step 5: 添加 ToolSummary 结构体**

```rust
/// 工具调用摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub tool_name: String,
    pub outcome: String,
    pub key_params: HashMap<String, String>,
}
```

- [ ] **Step 6: 添加 SegmentSummary 结构体**

```rust
/// LLM 生成的摘要（专注于"为什么"）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SegmentSummary {
    pub goal: String,
    pub progress: String,
    pub reasoning: String,
    pub remaining: String,
}
```

- [ ] **Step 7: 添加 MetadataIndex 结构体**

```rust
/// 关键元数据索引（确定性提取，不依赖 LLM）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataIndex {
    pub file_refs: Vec<FileRef>,
    pub symbol_refs: Vec<SymbolRef>,
    pub decisions: Vec<Decision>,
    pub tool_summaries: Vec<ToolSummary>,
}
```

- [ ] **Step 8: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-core`
Expected: 编译成功

- [ ] **Step 9: 提交**

```bash
git add crates/hermes-core/src/types.rs
git commit -m "feat(context): 添加元数据压缩相关类型"
```

---

## Task 2: 创建 MetadataExtractor 模块

**Files:**
- Create: `crates/hermes-core/src/metadata_extractor.rs`

- [ ] **Step 1: 创建文件结构**

```rust
//! MetadataExtractor - 确定性元数据提取器
//!
//! 从对话消息中提取关键元数据：文件路径、符号引用、决策记录、工具摘要。

use crate::{Decision, FileAction, FileRef, Message, MetadataIndex, SymbolKind, SymbolRef, ToolSummary};
use regex::Regex;
use std::collections::HashSet;

/// 确定性元数据提取器
pub struct MetadataExtractor {
    file_pattern: Regex,
    rust_symbol_pattern: Regex,
    decision_patterns: Vec<Regex>,
}

impl MetadataExtractor {
    pub fn new() -> Self {
        Self {
            file_pattern: Regex::new(r"([a-zA-Z0-9_\-./]+\.(rs|toml|json|yaml|yml|txt|md|sh|py|js|ts))").unwrap(),
            rust_symbol_pattern: Regex::new(r"`([a-zA-Z_][a-zA-Z0-9_]*)`").unwrap(),
            decision_patterns: vec![
                Regex::new(r"决定用\s+(\S+)").unwrap(),
                Regex::new(r"选择\s+(\S+)").unwrap(),
                Regex::new(r"采用了\s+(\S+)").unwrap(),
                Regex::new(r"使用\s+(\S+)\s+因为").unwrap(),
            ],
        }
    }

    /// 从消息列表中提取元数据
    pub fn extract(&self, messages: &[Message]) -> MetadataIndex {
        MetadataIndex {
            file_refs: self.extract_file_refs(messages),
            symbol_refs: self.extract_symbol_refs(messages),
            decisions: self.extract_decisions(messages),
            tool_summaries: self.extract_tool_summaries(messages),
        }
    }
}
```

- [ ] **Step 2: 实现 extract_file_refs 方法**

```rust
/// 提取文件引用
fn extract_file_refs(&self, messages: &[Message]) -> Vec<FileRef> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();

    for msg in messages {
        let content = self.message_content(msg);
        for cap in self.file_pattern.captures_iter(content) {
            let path = cap.get(1).map_or("", |m| m.as_str()).to_string();
            if path.len() > 3 && !seen.contains(&path) {
                seen.insert(path.clone());
                let action = self.infer_file_action(msg, &path);
                refs.push(FileRef {
                    path,
                    action,
                    snippet: None,
                });
            }
        }
    }

    refs
}

fn message_content(&self, msg: &Message) -> &str {
    match &msg.content {
        crate::Content::Text(s) => s,
        crate::Content::ToolResult { content, .. } => content,
        crate::Content::Image { .. } => "",
    }
}

fn infer_file_action(&self, msg: &Message, path: &str) -> FileAction {
    let content = self.message_content(msg);
    let lower = content.to_lowercase();
    if lower.contains("create") || lower.contains("新建") || lower.contains("写入") {
        if lower.contains("modified") || lower.contains("更新") {
            FileAction::Modified
        } else {
            FileAction::Created
        }
    } else if lower.contains("delete") || lower.contains("删除") {
        FileAction::Deleted
    } else if lower.contains("write") || lower.contains("写入") || lower.contains("保存") {
        FileAction::Write
    } else {
        FileAction::Read
    }
}
```

- [ ] **Step 3: 实现 extract_symbol_refs 方法**

```rust
/// 提取符号引用
fn extract_symbol_refs(&self, messages: &[Message]) -> Vec<SymbolRef> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();

    // Rust 符号模式
    let rust_fn = Regex::new(r"(?:fn|struct|impl|trait|enum|type)\s+([A-Z][a-zA-Z0-9_]*)").unwrap();
    let snake_case = Regex::new(r"`([a-z_][a-z0-9_]+)`").unwrap();

    for msg in messages {
        let content = self.message_content(msg);

        // 匹配大写开头的符号（类型）
        for cap in rust_fn.captures_iter(content) {
            if let Some(name) = cap.get(1) {
                let name_str = name.as_str().to_string();
                if !seen.contains(&name_str) && name_str.len() > 2 {
                    seen.insert(name_str.clone());
                    let kind = self.infer_symbol_kind(&name_str);
                    refs.push(SymbolRef {
                        name: name_str,
                        kind,
                        file_path: String::new(),
                        line: None,
                    });
                }
            }
        }

        // 匹配 snake_case 函数
        for cap in snake_case.captures_iter(content) {
            if let Some(name) = cap.get(1) {
                let name_str = name.as_str().to_string();
                if !seen.contains(&name_str) && name_str.len() > 2 {
                    seen.insert(name_str.clone());
                    refs.push(SymbolRef {
                        name: name_str,
                        kind: SymbolKind::Function,
                        file_path: String::new(),
                        line: None,
                    });
                }
            }
        }
    }

    refs
}

fn infer_symbol_kind(&self, name: &str) -> SymbolKind {
    if name.starts_with("I") || name.starts_with("Trait") {
        SymbolKind::Trait
    } else if name.ends_with("Impl") || name.starts_with("impl") {
        SymbolKind::Impl
    } else if name.ends_with("Error") || name.ends_with("Result") {
        SymbolKind::Type
    } else {
        SymbolKind::Struct
    }
}
```

- [ ] **Step 4: 实现 extract_decisions 方法**

```rust
/// 提取决策
fn extract_decisions(&self, messages: &[Message]) -> Vec<Decision> {
    let mut decisions = Vec::new();

    for msg in messages {
        let content = self.message_content(msg);

        for pattern in &self.decision_patterns {
            if let Some(cap) = pattern.captures(content) {
                if let Some(chosen) = cap.get(1) {
                    decisions.push(Decision {
                        description: self.extract_decision_context(content),
                        chosen_option: chosen.as_str().to_string(),
                        alternatives: Vec::new(),
                        rationale: String::new(),
                    });
                }
            }
        }
    }

    decisions
}

fn extract_decision_context(&self, content: &str) -> String {
    // 提取决策附近的上下文
    let len = content.len().min(200);
    content.chars().take(len).collect()
}
```

- [ ] **Step 5: 实现 extract_tool_summaries 方法**

```rust
/// 提取工具摘要
fn extract_tool_summaries(&self, messages: &[Message]) -> Vec<ToolSummary> {
    let mut summaries = Vec::new();
    let mut seen = HashSet::new();

    for msg in messages {
        if let Some(tool_name) = &msg.tool_name {
            if seen.contains(tool_name) {
                continue;
            }
            seen.insert(tool_name.clone());

            let outcome = match &msg.content {
                crate::Content::Text(s) => {
                    if s.contains("error") || s.contains("失败") {
                        "failed".to_string()
                    } else {
                        s.chars().take(100).collect()
                    }
                }
                crate::Content::ToolResult { content, .. } => {
                    if content.contains("error") || content.contains("失败") {
                        "failed".to_string()
                    } else {
                        content.chars().take(100).collect()
                    }
                }
                crate::Content::Image { .. } => "image processed".to_string(),
            };

            summaries.push(ToolSummary {
                tool_name: tool_name.clone(),
                outcome,
                key_params: std::collections::HashMap::new(),
            });
        }
    }

    summaries
}
```

- [ ] **Step 6: 添加 Default 实现**

```rust
impl Default for MetadataExtractor {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 7: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-core`
Expected: 编译成功

- [ ] **Step 8: 提交**

```bash
git add crates/hermes-core/src/metadata_extractor.rs
git commit -m "feat(context): 添加 MetadataExtractor 元数据提取器"
```

---

## Task 3: 更新 ContextCompressor 使用新结构

**Files:**
- Modify: `crates/hermes-core/src/context_compressor.rs`

- [ ] **Step 1: 添加 MetadataExtractor 到导入**

```rust
use crate::metadata_extractor::MetadataExtractor;
use crate::{SegmentSummary, MetadataIndex};
```

- [ ] **Step 2: 添加 extractor 字段到 ContextCompressor**

```rust
pub struct ContextCompressor {
    /// LLM Provider 用于生成摘要
    llm: Arc<dyn LlmProvider>,
    // ... existing fields ...

    /// 元数据提取器
    extractor: MetadataExtractor,
}
```

- [ ] **Step 3: 更新 new() 方法初始化 extractor**

```rust
impl ContextCompressor {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        model: String,
        context_length: usize,
    ) -> Self {
        // ... existing initialization ...

        Self {
            // ... existing fields ...
            extractor: MetadataExtractor::new(),
        }
    }
}
```

- [ ] **Step 4: 更新 generate_summary 生成 SegmentSummary**

```rust
/// 生成结构化摘要（goal/progress/reasoning/remaining）
async fn generate_summary(
    &self,
    turns_to_summarize: &[Message],
    metadata: &MetadataIndex,
) -> Result<SegmentSummary, String> {
    let content_to_summarize = self.serialize_for_summary(turns_to_summarize);

    // 构建已提取的元数据列表
    let file_list: Vec<String> = metadata.file_refs
        .iter()
        .map(|f| format!("- {} ({})", f.path, format!("{:?}", f.action)))
        .collect();
    let symbol_list: Vec<String> = metadata.symbol_refs
        .iter()
        .map(|s| format!("- [{}] {}", format!("{:?}", s.kind), s.name))
        .collect();

    let prompt = if let Some(prev) = &self.previous_summary {
        format!(
            "You are a context compression assistant. Create a structured summary.\n\n\
            PREVIOUS SUMMARY:\n{}\n\n\
            NEW TURNS:\n{}\n\n\
            Already extracted metadata (DO NOT repeat in summary):\nFiles: {}\nSymbols: {}\n\n\
            Output ONLY valid JSON with this structure:\n{{\"goal\": \"...\", \"progress\": \"...\", \"reasoning\": \"...\", \"remaining\": \"...\"}}",
            prev,
            content_to_summarize,
            file_list.join("\n"),
            symbol_list.join("\n")
        )
    } else {
        format!(
            "You are a context compression assistant. Create a structured summary.\n\n\
            TURNS TO SUMMARIZE:\n{}\n\n\
            Already extracted metadata (include in summary if relevant):\nFiles: {}\nSymbols: {}\n\n\
            Output ONLY valid JSON with this structure:\n{{\"goal\": \"...\", \"progress\": \"...\", \"reasoning\": \"...\", \"remaining\": \"...\"}}",
            content_to_summarize,
            file_list.join("\n"),
            symbol_list.join("\n")
        )
    };

    let summary_budget = self.compute_summary_budget(turns_to_summarize);

    let request = ChatRequest {
        model: ModelId::new("summary", "internal"),
        messages: vec![Message::user(prompt)],
        tools: None,
        system_prompt: None,
        temperature: Some(0.3),
        max_tokens: Some(summary_budget * 2),
    };

    let response = self.llm.chat(request).await
        .map_err(|e| e.to_string())?;

    // 解析 JSON 响应
    serde_json::from_str::<SegmentSummary>(&response.content)
        .map_err(|e| format!("Failed to parse summary: {} - Response was: {}", e, response.content))
}
```

- [ ] **Step 5: 更新 compress() 方法使用元数据提取**

```rust
pub async fn compress(
    &mut self,
    messages: Vec<Message>,
    _current_tokens: Option<usize>,
    _focus_topic: Option<&str>,
) -> Result<Vec<Message>, String> {
    let n_messages = messages.len();
    let min_for_compress = self.protect_first_n + 4;

    if n_messages <= min_for_compress {
        return Ok(messages);
    }

    // Phase 1: 剪枝旧工具结果
    let messages = self.prune_old_tool_results(&messages);

    // Phase 2: 确定边界
    let compress_start = self.protect_first_n;
    let compress_end = self.find_tail_cut_by_tokens(&messages, compress_start);

    if compress_start >= compress_end {
        return Ok(messages);
    }

    let turns_to_summarize = messages[compress_start..compress_end].to_vec();

    // Phase 2.5: 元数据提取（确定性，无 LLM 调用）
    let metadata = self.extractor.extract(&turns_to_summarize);

    // Phase 3: 生成结构化摘要
    let summary = match self.generate_summary(&turns_to_summarize, &metadata).await {
        Ok(s) => s,
        Err(_e) => {
            SegmentSummary::default()
        }
    };

    // Phase 4: 组装压缩后的消息列表
    let mut compressed = Vec::new();

    // 添加保护的头部消息
    for i in 0..compress_start {
        compressed.push(messages[i].clone());
    }

    // 添加摘要作为用户消息
    let summary_content = format!(
        "## 压缩摘要\n\n### 引用的文件\n{}\n\n### 符号引用\n{}\n\n### 目标与进度\nGoal: {}\nProgress: {}\nReasoning: {}\nRemaining: {}",
        metadata.file_refs.iter().map(|f| format!("- [{:?}] {}", f.action, f.path)).collect::<Vec<_>>().join("\n"),
        metadata.symbol_refs.iter().map(|s| format!("- [{:?}] {}", s.kind, s.name)).collect::<Vec<_>>().join("\n"),
        summary.goal,
        summary.progress,
        summary.reasoning,
        summary.remaining,
    );

    let summary_msg = Message {
        role: Role::User,
        content: Content::Text(summary_content),
        reasoning: None,
        tool_call_id: None,
        tool_name: None,
    };
    compressed.push(summary_msg);

    // 添加保护的尾部消息
    for i in compress_end..n_messages {
        compressed.push(messages[i].clone());
    }

    // 清理孤立的工具调用/结果对
    let compressed = self.sanitize_tool_pairs(compressed);

    self.compression_count += 1;
    self.previous_summary = Some(serde_json::to_string(&summary).unwrap_or_default());

    Ok(compressed)
}
```

- [ ] **Step 6: 更新 ContextEngine 实现**

```rust
#[async_trait]
impl ContextEngine for ContextCompressor {
    // ... existing methods ...

    async fn compress(
        &self,
        messages: &[Message],
        prompt_tokens: usize,
        focus_topic: Option<&str>,
    ) -> Result<Vec<Message>, ToolError> {
        let mut self_clone = Self {
            llm: self.llm.clone(),
            model: self.model.clone(),
            context_length: self.context_length,
            threshold_percent: self.threshold_percent,
            summary_target_ratio: self.summary_target_ratio,
            protect_first_n: self.protect_first_n,
            protect_last_n: self.protect_last_n,
            tail_token_budget: self.tail_token_budget,
            max_summary_tokens: self.max_summary_tokens,
            compression_count: self.compression_count,
            previous_summary: self.previous_summary.clone(),
            extractor: MetadataExtractor::new(), // 新增
        };
        ContextCompressor::compress(&mut self_clone, messages.to_vec(), Some(prompt_tokens), focus_topic)
            .await
            .map_err(ToolError::Execution)
    }
}
```

- [ ] **Step 7: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-core`
Expected: 编译成功

- [ ] **Step 8: 提交**

```bash
git add crates/hermes-core/src/context_compressor.rs
git commit -m "feat(context): 集成 MetadataExtractor 到压缩器"
```

---

## Task 4: 更新 CompressedSegment 结构

**Files:**
- Modify: `crates/hermes-memory/src/compressed.rs`

- [ ] **Step 1: 更新 CompressedSegment 结构体**

```rust
//! Compressed message segment structure

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 文件操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAction {
    Read,
    Write,
    Created,
    Modified,
    Deleted,
}

/// 符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Struct,
    Trait,
    Impl,
    Enum,
    Module,
    Type,
    Constant,
}

/// 单个文件引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub action: FileAction,
    pub snippet: Option<String>,
}

/// 符号引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub line: Option<u32>,
}

/// 决策记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub description: String,
    pub chosen_option: String,
    pub alternatives: Vec<String>,
    pub rationale: String,
}

/// 工具调用摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    pub tool_name: String,
    pub outcome: String,
    pub key_params: HashMap<String, String>,
}

/// 关键元数据索引
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataIndex {
    pub file_refs: Vec<FileRef>,
    pub symbol_refs: Vec<SymbolRef>,
    pub decisions: Vec<Decision>,
    pub tool_summaries: Vec<ToolSummary>,
}

/// LLM 生成的摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSummary {
    pub goal: String,
    pub progress: String,
    pub reasoning: String,
    pub remaining: String,
}

impl Default for SegmentSummary {
    fn default() -> Self {
        Self {
            goal: String::new(),
            progress: String::new(),
            reasoning: String::new(),
            remaining: String::new(),
        }
    }
}

/// A compressed segment of messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedSegment {
    pub id: String,
    pub session_id: String,
    pub start_message_id: i64,
    pub end_message_id: i64,
    pub metadata: MetadataIndex,
    pub summary: SegmentSummary,
    pub vector: Vec<f32>,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 2: 更新 new() 方法**

```rust
impl CompressedSegment {
    pub fn new(
        session_id: String,
        start_message_id: i64,
        end_message_id: i64,
        metadata: MetadataIndex,
        summary: SegmentSummary,
        vector: Vec<f32>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            start_message_id,
            end_message_id,
            metadata,
            summary,
            vector,
            created_at: Utc::now(),
        }
    }
}
```

- [ ] **Step 3: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-memory`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-memory/src/compressed.rs
git commit -m "feat(memory): 更新 CompressedSegment 支持元数据索引"
```

---

## Task 5: 添加 MetadataExtractor 测试

**Files:**
- Create: `crates/hermes-core/tests/test_metadata_extractor.rs`

- [ ] **Step 1: 编写文件引用提取测试**

```rust
#[cfg(test)]
mod tests {
    use hermes_core::metadata_extractor::MetadataExtractor;
    use hermes_core::{Message, Content, Role};

    #[test]
    fn test_extract_file_refs() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: Content::Text("Read src/main.rs and Cargo.toml".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.file_refs.is_empty());
        assert!(metadata.file_refs.iter().any(|f| f.path.contains("main.rs")));
    }

    #[test]
    fn test_extract_no_files() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: Content::Text("Hello world".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(metadata.file_refs.is_empty());
    }

    #[test]
    fn test_extract_symbol_refs() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: Content::Text("I implemented `ContextCompressor` struct".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.symbol_refs.is_empty());
    }

    #[test]
    fn test_extract_decisions() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: Content::Text("决定用 Arc<RwLock<T>> 来共享状态".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.decisions.is_empty());
    }

    #[test]
    fn test_extract_tool_summaries() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::Tool,
                content: Content::ToolResult {
                    tool_call_id: "call_1".to_string(),
                    content: "File read successfully".to_string(),
                },
                reasoning: None,
                tool_call_id: Some("call_1".to_string()),
                tool_name: Some("ReadFile".to_string()),
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.tool_summaries.is_empty());
        assert_eq!(metadata.tool_summaries[0].tool_name, "ReadFile");
    }

    #[test]
    fn test_extract_empty_messages() {
        let extractor = MetadataExtractor::new();
        let messages: Vec<Message> = vec![];

        let metadata = extractor.extract(&messages);
        assert!(metadata.file_refs.is_empty());
        assert!(metadata.symbol_refs.is_empty());
        assert!(metadata.decisions.is_empty());
        assert!(metadata.tool_summaries.is_empty());
    }
}
```

- [ ] **Step 2: 运行测试验证**

Run: `cargo test -p hermes-core test_metadata_extractor`
Expected: 所有测试通过

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-core/tests/test_metadata_extractor.rs
git commit -m "test(context): 添加 MetadataExtractor 测试"
```

---

## Task 6: 运行完整测试

- [ ] **Step 1: 运行 hermes-core 测试**

Run: `cargo test -p hermes-core 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 2: 运行 hermes-memory 测试**

Run: `cargo test -p hermes-memory 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 3: 运行所有相关测试**

Run: `cargo test -p hermes-core -p hermes-memory -p hermes-cli 2>&1 | grep "test result"`
Expected: 所有测试通过

- [ ] **Step 4: 提交最终变更**

```bash
git add -A
git commit -m "feat(context): 完成上下文压缩混合方案实现

- MetadataExtractor 确定性提取文件/符号/决策/工具
- ContextCompressor 生成 goal/progress/reasoning/remaining 摘要
- CompressedSegment 支持元数据索引存储
- 向后兼容现有触发机制
Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## 成功标准检查清单

- [ ] MetadataExtractor 能正确提取 .rs, .toml, .json 等文件路径
- [ ] MetadataExtractor 能识别 Rust 符号（大写开头的类型、snake_case 函数）
- [ ] MetadataExtractor 能检测决策语言（决定用、选择、采用了）
- [ ] 压缩后的摘要包含 goal/progress/reasoning/remaining
- [ ] 解压内容比原有 summary 包含更多可追溯信息
- [ ] 现有压缩触发机制正常工作
- [ ] 向后兼容：现有 CompressedSegment 正常处理
