//! 类型定义模块
//!
//! 本模块定义了 hermes-core 中所有核心数据类型，是整个库的基础类型层。
//!
//! ## 主要类型
//! - **角色与内容**: `Role`（消息角色）、`Content`（消息内容，支持文本/图片/工具结果）
//! - **消息**: `Message` — 包含角色、内容、推理内容、工具调用信息的完整消息结构
//! - **工具调用**: `ToolCall`（工具调用请求）、`ToolDefinition`（工具的 schema 定义）
//! - **LLM 请求/响应**: `ChatRequest`、`ChatResponse`、`FinishReason`、`Usage`
//! - **模型标识**: `ModelId` — 格式为 `provider/model-name`（如 `openai/gpt-4o`）
//! - **工具上下文**: `ToolContext` — 工具执行时的会话和工作目录信息
//! - **流式回调**: `StreamingCallback` — 用于处理流式响应的回调函数类型
//!
//! ## 与其他模块的关系
//! - 被 `provider.rs`（LLM Provider 的请求/响应类型）
//! - 被 `agent.rs`（Agent 的消息构建和解析）
//! - 被 `conversation.rs`（会话请求/响应包装）
//! - 被 `gateway.rs`（入站消息的原始数据存储）
//! - 被 `tool_dispatcher.rs`（工具调用的定义和分发）

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

// =============================================================================
// Role & Content
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Image {
        url: String,
        detail: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        content: String,
    },
}

// =============================================================================
// Message
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Content,
    pub reasoning: Option<String>,
    pub tool_call_id: Option<String>,
    pub tool_name: Option<String>,
}

impl Message {
    pub fn user(content: impl Into<Content>) -> Self {
        Message {
            role: Role::User,
            content: content.into(),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    pub fn assistant(content: impl Into<Content>) -> Self {
        Message {
            role: Role::Assistant,
            content: content.into(),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    pub fn system(content: impl Into<Content>) -> Self {
        Message {
            role: Role::System,
            content: content.into(),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<Content>) -> Self {
        Message {
            role: Role::Tool,
            content: content.into(),
            reasoning: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_name: None,
        }
    }
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::Text(s)
    }
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::Text(s.to_string())
    }
}

impl Content {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text(s) => Some(s),
            _ => None,
        }
    }
}

// =============================================================================
// ToolCall & ToolDefinition
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// =============================================================================
// FinishReason & Usage
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    Other,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<usize>,
}

// =============================================================================
// ChatResponse & ChatRequest
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub finish_reason: FinishReason,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub reasoning: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: ModelId,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub system_prompt: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
}

// =============================================================================
// ModelId
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId {
    pub provider: String,
    pub model: String,
}

impl ModelId {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return None;
        }
        Some(ModelId {
            provider: parts[0].to_string(),
            model: parts[1].to_string(),
        })
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

// =============================================================================
// ToolContext
// =============================================================================

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub working_directory: PathBuf,
    pub user_id: Option<String>,
    pub task_id: Option<String>,
    /// YOLO 模式 — 为 true 时跳过危险命令审批检查
    pub yolo_mode: bool,
    /// 文件检查点管理器 — 为 Some 时工具在修改文件前自动创建快照
    pub checkpoint_manager: Option<std::sync::Arc<hermes_checkpoint::CheckpointManager>>,
}

// =============================================================================
// StreamingCallback
// =============================================================================

pub type StreamingCallback = Box<dyn Fn(ChatResponse) + Send + Sync>;

// =============================================================================
// Context Compression Types (元数据压缩相关类型)
// =============================================================================

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

/// LLM 生成的分段摘要
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SegmentSummary {
    pub goal: String,
    pub progress: String,
    pub reasoning: String,
    pub remaining: String,
}

/// 元数据索引容器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataIndex {
    pub file_refs: Vec<FileRef>,
    pub symbol_refs: Vec<SymbolRef>,
    pub decisions: Vec<Decision>,
    pub tool_summaries: Vec<ToolSummary>,
}
