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

/// 消息角色枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// 系统消息（通常作为对话开始的角色设定）
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具调用结果消息
    Tool,
}

/// 消息内容枚举
///
/// 支持三种内容类型：纯文本、图片、工具调用结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// 纯文本内容
    Text(String),
    /// 图片内容
    Image {
        /// 图片 URL
        url: String,
        /// 图片细节级别（可选）
        detail: Option<String>,
    },
    /// 工具调用结果
    ToolResult {
        /// 关联的工具调用 ID
        tool_call_id: String,
        /// 工具返回的内容
        content: String,
    },
}

// =============================================================================
// Message
// =============================================================================

/// 对话消息结构体
///
/// 表示对话中的单条消息，包含角色、内容、推理过程和工具调用信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// 消息角色
    pub role: Role,
    /// 消息内容
    pub content: Content,
    /// 推理/思考内容（用于显示模型的思维过程）
    pub reasoning: Option<String>,
    /// 工具调用 ID（当角色为 Tool 时使用）
    pub tool_call_id: Option<String>,
    /// 工具名称（可选）
    pub tool_name: Option<String>,
}

impl Message {
    /// 创建用户消息
    pub fn user(content: impl Into<Content>) -> Self {
        Message {
            role: Role::User,
            content: content.into(),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// 创建助手消息
    pub fn assistant(content: impl Into<Content>) -> Self {
        Message {
            role: Role::Assistant,
            content: content.into(),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// 创建系统消息
    pub fn system(content: impl Into<Content>) -> Self {
        Message {
            role: Role::System,
            content: content.into(),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// 创建工具调用结果消息
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
    /// 尝试获取文本内容，如果不是文本则返回 None
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

/// 工具调用请求
///
/// 表示 LLM 发起的工具调用，包含调用 ID、工具名称和参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// 工具调用的唯一标识符
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 工具参数（键值对形式）
    pub arguments: HashMap<String, serde_json::Value>,
}

/// 工具定义
///
/// 描述一个工具的 schema，用于向 LLM 注册工具。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 工具参数的 JSON Schema
    pub parameters: serde_json::Value,
}

// =============================================================================
// FinishReason & Usage
// =============================================================================

/// LLM 响应的结束原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// 正常结束（模型完成生成）
    Stop,
    /// 长度超限（达到 max_tokens 或上下文窗口上限）
    Length,
    /// 内容过滤（触发安全过滤）
    ContentFilter,
    /// 其他原因
    Other,
}

/// Token 使用量统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// 输入 token 数量
    pub input_tokens: usize,
    /// 输出 token 数量
    pub output_tokens: usize,
    /// 缓存读取的 token 数量（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<usize>,
    /// 缓存写入的 token 数量（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<usize>,
    /// 推理 token 数量（可选，用于 o1 等模型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<usize>,
}

// =============================================================================
// ChatResponse & ChatRequest
// =============================================================================

/// LLM 聊天响应
///
/// 包含模型生成的内容、结束原因、工具调用和 token 使用量。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// 生成的内容
    pub content: String,
    /// 结束原因
    pub finish_reason: FinishReason,
    /// 工具调用列表（如果有）
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 推理/思考内容（可选）
    pub reasoning: Option<String>,
    /// Token 使用量统计
    pub usage: Option<Usage>,
}

/// LLM 聊天请求
///
/// 发送给 LLM 的完整请求，包含模型、消息、工具定义和生成参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// 模型标识符
    pub model: ModelId,
    /// 对话消息列表
    pub messages: Vec<Message>,
    /// 可用的工具定义列表
    pub tools: Option<Vec<ToolDefinition>>,
    /// 系统提示词
    pub system_prompt: Option<String>,
    /// 生成温度（0.0-2.0）
    pub temperature: Option<f32>,
    /// 最大生成 token 数
    pub max_tokens: Option<usize>,
}

// =============================================================================
// ModelId
// =============================================================================

/// 模型标识符
///
/// 格式为 `provider/model-name`，例如 `openai/gpt-4o`、`anthropic/claude-3-5-sonnet`。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId {
    /// 提供者名称（如 "openai"、"anthropic"）
    pub provider: String,
    /// 模型名称（如 "gpt-4o"、"claude-3-5-sonnet"）
    pub model: String,
}

impl ModelId {
    /// 创建新的模型标识符
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }

    /// 从字符串解析模型标识符
    ///
    /// 输入格式应为 `provider/model-name`。
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

/// 工具执行上下文
///
/// 提供工具执行时所需的会话和环境信息。
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// 会话 ID
    pub session_id: String,
    /// 工作目录
    pub working_directory: PathBuf,
    /// 用户 ID（可选）
    pub user_id: Option<String>,
    /// 任务 ID（可选）
    pub task_id: Option<String>,
    /// YOLO 模式 — 为 true 时跳过危险命令审批检查
    pub yolo_mode: bool,
    /// 文件检查点管理器 — 为 Some 时工具在修改文件前自动创建快照
    pub checkpoint_manager: Option<std::sync::Arc<hermes_checkpoint::CheckpointManager>>,
}

// =============================================================================
// StreamingCallback
// =============================================================================

/// 流式响应回调函数类型
pub type StreamingCallback = Box<dyn Fn(ChatResponse) + Send + Sync>;

// =============================================================================
// Context Compression Types (元数据压缩相关类型)
// =============================================================================

/// 文件操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAction {
    /// 读取文件
    Read,
    /// 写入文件
    Write,
    /// 创建文件
    Created,
    /// 修改文件
    Modified,
    /// 删除文件
    Deleted,
}

/// 符号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    /// 函数
    Function,
    /// 结构体
    Struct,
    /// Trait
    Trait,
    /// Impl 块
    Impl,
    /// 枚举
    Enum,
    /// 模块
    Module,
    /// 类型别名
    Type,
    /// 常量
    Constant,
}

/// 单个文件引用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    /// 文件路径
    pub path: String,
    /// 操作类型
    pub action: FileAction,
    /// 代码片段（可选）
    pub snippet: Option<String>,
}

/// 符号引用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRef {
    /// 符号名称
    pub name: String,
    /// 符号类型
    pub kind: SymbolKind,
    /// 所在文件路径
    pub file_path: String,
    /// 行号（可选）
    pub line: Option<u32>,
}

/// 决策记录
///
/// 记录 Agent 在对话中做出的重要决策。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    /// 决策描述
    pub description: String,
    /// 选择的方案
    pub chosen_option: String,
    /// 备选方案列表
    pub alternatives: Vec<String>,
    /// 选择理由
    pub rationale: String,
}

/// 工具调用摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    /// 工具名称
    pub tool_name: String,
    /// 执行结果
    pub outcome: String,
    /// 关键参数
    pub key_params: HashMap<String, String>,
}

/// LLM 生成的分段摘要
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SegmentSummary {
    /// 目标描述
    pub goal: String,
    /// 当前进度
    pub progress: String,
    /// 推理过程
    pub reasoning: String,
    /// 剩余任务
    pub remaining: String,
}

/// 元数据索引容器
///
/// 汇总对话中提取的所有元数据。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataIndex {
    /// 文件引用列表
    pub file_refs: Vec<FileRef>,
    /// 符号引用列表
    pub symbol_refs: Vec<SymbolRef>,
    /// 决策记录列表
    pub decisions: Vec<Decision>,
    /// 工具调用摘要列表
    pub tool_summaries: Vec<ToolSummary>,
}
