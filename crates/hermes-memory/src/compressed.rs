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

    /// Get the range of message IDs covered by this segment
    pub fn message_range(&self) -> (i64, i64) {
        (self.start_message_id, self.end_message_id)
    }

    /// Check if a message ID falls within this segment
    pub fn contains(&self, message_id: i64) -> bool {
        message_id >= self.start_message_id && message_id <= self.end_message_id
    }
}
