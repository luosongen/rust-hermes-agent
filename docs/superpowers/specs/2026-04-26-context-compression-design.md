# Context Compression 混合方案设计

> **Goal:** 改进上下文压缩质量，保留更多关键信息（文件路径、符号引用、决策结果），同时让 LLM 摘要专注于"为什么做"而非"做了什么"

> **Architecture:** 混合方案 — 确定性元数据提取 + LLM 生成"原因"摘要，双轨并行

> **Tech Stack:** Rust, serde, regex, hermes-core

---

## 1. 概述

### 1.1 当前状态

现有 `ContextCompressor` 的问题：
- LLM summary 丢失文件路径、函数名等关键引用
- 工具调用信息被简化，后续无法追溯
- 摘要格式不固定，Agent 难以依赖

### 1.2 目标

1. **保留元数据索引** — 确定性提取，不依赖 LLM
2. **改进摘要质量** — 专注于"为什么"，而非"是什么"
3. **可追溯** — 压缩后可还原关键引用

---

## 2. 架构设计

### 2.1 组件关系

```
┌─────────────────────────────────────────────────────┐
│                  ContextCompressor                     │
│  ┌─────────────────┐    ┌─────────────────────────┐ │
│  │ MetadataExtractor│───►│  CompressedSegment     │ │
│  │  (确定性提取)     │    │  - file_refs          │ │
│  └─────────────────┘    │  - symbol_refs        │ │
│                         │  - decision_refs      │ │
│  ┌─────────────────┐    │  - summary            │ │
│  │ LlmSummarizer   │───►│  - message_ids        │ │
│  │  (LLM 生成)     │    └─────────────────────────┘ │
│  └─────────────────┘                               │
└─────────────────────────────────────────────────────┘
```

### 2.2 数据结构

```rust
/// 关键元数据索引（确定性提取，不依赖 LLM）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataIndex {
    /// 引用过的文件路径
    pub file_refs: Vec<FileRef>,
    /// 引用过的符号（函数、结构体、Trait 等）
    pub symbol_refs: Vec<SymbolRef>,
    /// 决策记录
    pub decisions: Vec<Decision>,
    /// 工具调用摘要
    pub tool_summaries: Vec<ToolSummary>,
}

/// 单个文件引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub action: FileAction,  // Read, Write, Created, Modified
    pub snippet: Option<String>,  // 关键代码片段
}

/// 符号引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    pub name: String,
    pub kind: SymbolKind,  // Function, Struct, Trait, Impl, Enum
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
    pub outcome: String,  // 成功/失败/结果摘要
    pub key_params: HashMap<String, String>,  // 关键参数
}

/// 压缩后的段落
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedSegment {
    pub id: Option<i64>,
    pub session_id: String,
    pub message_ids: Vec<i64>,  // 原始消息 ID
    pub metadata: MetadataIndex,
    pub summary: SegmentSummary,
    pub created_at: DateTime<Utc>,
}

/// LLM 生成的摘要（专注于"为什么"）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSummary {
    /// 目标：这段对话要完成什么
    pub goal: String,
    /// 进度：已完成的工作
    pub progress: String,
    /// 原因：为什么做这些决定
    pub reasoning: String,
    /// 剩余：还需要什么
    pub remaining: String,
}
```

### 2.3 元数据提取器接口

```rust
/// 确定性元数据提取器
pub struct MetadataExtractor {
    // 提取模式配置
}

impl MetadataExtractor {
    /// 从消息列表中提取元数据
    pub fn extract(&self, messages: &[Message]) -> MetadataIndex;

    /// 提取文件引用
    fn extract_file_refs(&self, messages: &[Message]) -> Vec<FileRef> {
        // 1. 匹配文件路径模式
        //    - "src/**/*.rs"
        //    - "Cargo.toml"
        //    - "**/*.json"
        // 2. 确定操作类型（读/写/创建）
        // 3. 提取关键代码片段
    }

    /// 提取符号引用
    fn extract_symbol_refs(&self, messages: &[Message]) -> Vec<SymbolRef> {
        // 匹配 Rust 符号模式
        // - 函数: `fn_name` 或 `Module::function`
        // - 结构体: `StructName`
        // - Trait: `trait Name`
        // - impl: `impl Type`
    }

    /// 提取决策
    fn extract_decisions(&self, messages: &[Message]) -> Vec<Decision> {
        // 检测决策语言
        // - "决定用 X"
        // - "选择 Y"
        // - "采用了 Z"
        // - "因为...所以..."
    }

    /// 提取工具摘要
    fn extract_tool_summaries(&self, messages: &[Message]) -> Vec<ToolSummary> {
        // 从 tool_use 和 tool_result 消息中提取
    }
}
```

---

## 3. 压缩流程

### 3.1 压缩触发

沿用现有 `ContextPressureMonitor` 触发机制：
- **Normal (0-50%)**: 无操作
- **Moderate (50-75%)**: 准备压缩
- **High (75-90%)**: 建议压缩
- **Critical (90%+)**: 执行压缩

### 3.2 压缩步骤

```rust
impl ContextCompressor {
    pub async fn compress(
        &self,
        messages: Vec<Message>,
        max_tokens: Option<usize>,
    ) -> Result<CompressionResult, CompressionError> {
        // Step 1: 元数据提取（确定性，无 LLM 调用）
        let metadata = self.extractor.extract(&messages);

        // Step 2: 确定保留范围
        // - 头部：system prompt + 第一轮
        // - 尾部：最近 N 条消息
        // - 中间：待压缩
        let (head, middle, tail) = self.split_messages(messages);

        // Step 3: 如果中间部分超过阈值，生成 LLM 摘要
        let summary = if middle.len() > self.summary_threshold {
            self.llm_summarize(middle, &metadata).await?
        } else {
            SegmentSummary::default()
        };

        // Step 4: 构建 CompressedSegment
        let segment = CompressedSegment {
            id: None,
            session_id: self.session_id.clone(),
            message_ids: middle.iter().map(|m| m.id).collect(),
            metadata,
            summary,
            created_at: Utc::now(),
        };

        // Step 5: 返回压缩结果
        Ok(CompressionResult {
            head_messages: head,
            compressed_segment: segment,
            tail_messages: tail,
        })
    }
}
```

### 3.3 LLM Summary Prompt

```
你是一个对话压缩助手。请将以下对话压缩为结构化摘要。

要求：
- "已完成决策"：列出具体决策及结果（文件、配置、架构选择）
- "保留的上下文"：列出关键文件路径和符号引用（已有元数据索引）
- "进度"：简洁描述已完成的工作
- "未完成的工作"：描述剩余步骤

重要：
- 不要重复已在元数据中的文件路径
- 不要列举工具调用参数，只描述结果
- 专注于"为什么做"而非"做了什么"
- 保持摘要简洁，不超过 500 tokens
```

---

## 4. 恢复流程

### 4.1 解压时显示给 Agent 的内容

```rust
fn decompress(segment: &CompressedSegment) -> String {
    let mut output = String::new();

    output.push_str("## 压缩摘要\n\n");

    // 元数据索引
    if !segment.metadata.file_refs.is_empty() {
        output.push_str("### 引用的文件\n");
        for FileRef { path, action, .. } in &segment.metadata.file_refs {
            output.push_str(&format!("- [{}] {}\n", action, path));
        }
        output.push('\n');
    }

    if !segment.metadata.symbol_refs.is_empty() {
        output.push_str("### 符号引用\n");
        for SymbolRef { name, kind, .. } in &segment.metadata.symbol_refs {
            output.push_str(&format!("- [{}] {}\n", kind, name));
        }
        output.push('\n');
    }

    if !segment.metadata.decisions.is_empty() {
        output.push_str("### 决策\n");
        for Decision { description, chosen_option, .. } in &segment.metadata.decisions {
            output.push_str(&format!("- {} → {}\n", description, chosen_option));
        }
        output.push('\n');
    }

    // LLM 摘要
    output.push_str("### 目标与进度\n");
    output.push_str(&format!("Goal: {}\n", segment.summary.goal));
    output.push_str(&format!("Progress: {}\n", segment.summary.progress));
    output.push_str(&format!("Reasoning: {}\n", segment.summary.reasoning));
    output.push_str(&format!("Remaining: {}\n", segment.summary.remaining));

    output
}
```

### 4.2 示例输出

```markdown
## 压缩摘要

### 引用的文件
- [Read] src/hermes-core/src/context_compressor.rs
- [Write] crates/hermes-skills/src/executor.rs

### 符号引用
- [Struct] ContextCompressor
- [Struct] CompressedSegment
- [Function] MetadataExtractor::extract

### 决策
- 使用 Arc<RwLock<SkillManager>> → 管理器共享
- 扩展 skill_execute 工具 → 支持 start/complete/status

### 目标与进度
Goal: 实现技能主动执行功能
Progress: 完成了 SkillExecutor 和 parse_steps
Reasoning: 因为需要跟踪步骤状态和进度
Remaining: 还需要扩展 tool 为每个步骤生成执行计划
```

---

## 5. 文件变更

| 文件 | 变更 |
|------|------|
| `crates/hermes-core/src/context_compressor.rs` | 新增 MetadataExtractor，重构 compress() |
| `crates/hermes-core/src/types.rs` | 新增 MetadataIndex, FileRef, SymbolRef, Decision, ToolSummary, SegmentSummary |
| `crates/hermes-memory/src/compressed.rs` | 更新 CompressedSegment 结构 |

---

## 6. 依赖

无新依赖。使用现有的：
- `serde`
- `regex` (用于模式匹配)
- `chrono` (时间戳)

---

## 7. 成功标准

1. `MetadataExtractor` 能正确提取文件路径、符号引用
2. 压缩后的摘要包含 goal/progress/reasoning/remaining
3. 解压内容比原有 summary 包含更多可追溯信息
4. 现有压缩触发机制正常工作
5. 向后兼容：现有无压缩的 CompressedSegment 正常处理
