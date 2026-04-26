# Context Compression 设计文档

## 概述

在 `hermes-memory` crate 中实现 Context Compression（上下文压缩）功能，允许 Agent 在对话历史较长时自动总结和压缩之前的消息内容，以保持在 LLM 的 token 限制内。

## 目标

- 当对话 token 数量达到阈值时，自动压缩历史消息
- 支持云端 LLM API（OpenAI）和本地模型（Ollama）生成摘要
- 将原始消息转换为向量存储，支持检索时展开

## 架构

```
hermes-memory/
├── src/
│   ├── lib.rs
│   ├── store.rs              # SqliteSessionStore
│   ├── compression.rs         # 新增：CompressionManager 核心逻辑
│   ├── summarizer.rs          # 新增：摘要生成器（LLM API / Ollama）
│   ├── compressed.rs          # 新增：CompressedMessage 结构
│   └── error.rs               # 新增：CompressionError
```

## 触发条件

- **Token 阈值**：当 token 数量 ≥ 8000（可配置）时触发
- **消息数量阈值**：当消息数量 ≥ 50 条（可配置）时触发
- **最小压缩单元**：5 条连续消息

## 数据流

```
用户消息 → Agent.append_message()
              ↓
        检查 token_count ≥ 阈值？
              ↓是
        获取 [session_start → latest] 消息
              ↓
        调用 Summarizer 生成摘要 + 向量
              ↓
        存储 CompressedMessage，标记原始消息为 "compressed"
              ↓
        保留首尾消息，删除中间原始消息
```

## 检索流程

```
Agent.get_messages()
        ↓
  查找压缩段 → 返回压缩消息 + 摘要
        ↓
  向量相似度检索（可选）
        ↓
  展开相关压缩段返回给 LLM
```

## 核心组件

### CompressionManager

压缩管理器，负责：
- 检测触发条件
- 协调压缩流程
- 管理压缩策略配置

```rust
pub struct CompressionManager {
    config: CompressionConfig,
    summarizer: Summarizer,
    store: Arc<SqliteSessionStore>,
}

impl CompressionManager {
    pub fn should_compress(&self, session_id: &str) -> bool;
    pub async fn compress(&self, session_id: &str) -> Result<(), CompressionError>;
}
```

### Summarizer

摘要生成器，支持可配置的 LLM 提供者：

```rust
pub enum LlmProvider {
    OpenAi { model: String },
    Ollama { url: String, model: String },
}

pub struct Summarizer {
    provider: LlmProvider,
    http_client: reqwest::Client,
}

impl Summarizer {
    pub async fn summarize(&self, messages: &[Message]) -> Result<SummarizedChunk, CompressionError>;
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, CompressionError>;
}
```

### CompressedMessage

压缩消息结构：

```rust
pub struct CompressedSegment {
    pub id: String,
    pub session_id: String,
    pub start_message_id: String,
    pub end_message_id: String,
    pub summary: String,           // LLM 生成的摘要
    pub vector: Vec<f32>,          // 消息向量的 embedding
    pub created_at: DateTime<Utc>,
}
```

## 数据库 Schema

```sql
-- 压缩消息段
CREATE TABLE compressed_segments (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    start_message_id TEXT NOT NULL,
    end_message_id TEXT NOT NULL,
    summary TEXT NOT NULL,
    vector BLOB NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);

-- 消息表新增标记
ALTER TABLE messages ADD COLUMN compressed BOOLEAN DEFAULT FALSE;
```

## 配置项

```toml
[context_compression]
enabled = true
token_threshold = 8000
message_count_threshold = 50
compression_mode = "hybrid"  # "summary_only" | "vector_only" | "hybrid"

[summarizer]
provider = "openai"           # "openai" | "ollama"
model = "gpt-4o-mini"
ollama_url = "http://localhost:11434"
```

## 错误处理

```rust
pub enum CompressionError {
    LlmApi(String),
    VectorStore(String),
    SessionNotFound(String),
    MessageNotFound(String),
    Config(String),
}
```

**降级策略：**
- LLM API 失败：记录警告，跳过本次压缩，继续使用原始消息
- 向量存储失败：降级为"仅摘要"模式
- 数据库错误：返回错误给 Agent

## 验证规则

| 规则 | 限制 |
|------|------|
| 最小压缩单元 | 5 条连续消息 |
| 最大摘要长度 | 500 tokens |
| 向量维度 | 1536（OpenAI ada-002）或 768（Ollama） |
| 保留消息 | 每段压缩保留首条和末条 |

## 与现有模块集成

- `hermes-memory`：新增 `CompressionManager`、`Summarizer`、`CompressedSegment`
- `hermes-core`：`Agent` 持有 `CompressionManager` 引用，在 `append_message` 后触发检查
- 配置系统：使用 `figment` 读取 `context_compression` 和 `summarizer` 配置

## 实现顺序

1. `compression.rs` - CompressionManager 核心逻辑
2. `summarizer.rs` - Summarizer 摘要生成器
3. `compressed.rs` - CompressedSegment 结构
4. `error.rs` - CompressionError 错误类型
5. 扩展 `SqliteSessionStore` 支持压缩消息
6. 扩展 `Agent` 集成压缩触发
7. 测试
