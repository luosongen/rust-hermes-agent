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

    fn message_content<'a>(&self, msg: &'a Message) -> &'a str {
        match &msg.content {
            crate::Content::Text(s) => s,
            crate::Content::ToolResult { content, .. } => content,
            crate::Content::Image { .. } => "",
        }
    }

    fn infer_file_action(&self, msg: &Message, _path: &str) -> FileAction {
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
}

impl Default for MetadataExtractor {
    fn default() -> Self {
        Self::new()
    }
}