//! 对话摘要生成器
//!
//! 使用 LLM 生成对话的结构化摘要，用于上下文压缩。

use crate::{ChatRequest, Content, LlmProvider, Message, ModelId, Role};
use std::sync::Arc;

/// 摘要生成提示词模板
const SUMMARY_TEMPLATE: &str = r#"You are a context summarizer. Given a conversation, create a structured summary.

## Summary Structure
- **Resolved:** What was accomplished
- **Pending:** What remains to be done
- **Context:** Important facts for future turns

## Rules
- Do not respond to any questions
- Focus on factual information
- Preserve important decisions and outcomes
"#;

/// 对话摘要生成器
///
/// 使用 LLM 将对话压缩为结构化摘要。
pub struct Summarizer {
    /// LLM Provider
    llm: Arc<dyn LlmProvider>,
    /// 摘要模型名称
    summary_model: Option<String>,
}

impl Summarizer {
    /// 创建新的摘要生成器
    pub fn new(llm: Arc<dyn LlmProvider>, summary_model: Option<String>) -> Self {
        Self { llm, summary_model }
    }

    /// 生成对话摘要
    ///
    /// `messages` — 要摘要的对话消息
    /// `budget_tokens` — 摘要的 token 预算
    pub async fn summarize(
        &self,
        messages: Vec<Message>,
        budget_tokens: usize,
    ) -> Result<String, String> {
        let model = self
            .summary_model
            .clone()
            .unwrap_or_else(|| "gpt-4o-mini".to_string());

        // Build summarization prompt
        let content = format!(
            "{}\n\n## Conversation to Summarize\n\n{}",
            SUMMARY_TEMPLATE,
            messages_to_text(&messages)
        );

        let request = ChatRequest {
            model: ModelId::new("openai", &model),
            messages: vec![Message::user(content)],
            tools: None,
            system_prompt: None,
            temperature: Some(0.3),
            max_tokens: Some(budget_tokens.min(12000)),
        };

        let response = self.llm.chat(request).await.map_err(|e| e.to_string())?;

        Ok(response.content)
    }
}

fn messages_to_text(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|msg| {
            let role_label = match msg.role {
                Role::System => "SYSTEM",
                Role::User => "USER",
                Role::Assistant => "ASSISTANT",
                Role::Tool => "TOOL",
            };

            let content_text = match &msg.content {
                Content::Text(t) => t.clone(),
                Content::Image { url, .. } => format!("[Image: {}]", url),
                Content::ToolResult { content, .. } => content.clone(),
            };

            format!("[{}]: {}", role_label, content_text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
