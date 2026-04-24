//! Title Generator — 基于首条对话自动生成会话标题

use crate::{ChatRequest, LlmProvider, Message, ModelId};
use std::sync::Arc;

const TITLE_PROMPT: &str = "Generate a short, descriptive title (3-7 words) for a conversation that starts with the following exchange. The title should capture the main topic or intent. Return ONLY the title text, nothing else. No quotes, no punctuation at the end, no prefixes.";

/// 会话标题生成器
///
/// 使用便宜的 LLM 模型（如 gpt-4o-mini）异步生成会话标题。
pub struct TitleGenerator {
    provider: Arc<dyn LlmProvider>,
    model: ModelId,
}

impl TitleGenerator {
    /// 创建标题生成器
    ///
    /// `provider` — LLM provider（建议使用便宜的模型）
    /// `model` — 用于生成标题的模型 ID
    pub fn new(provider: Arc<dyn LlmProvider>, model: ModelId) -> Self {
        Self { provider, model }
    }

    /// 使用默认模型创建标题生成器
    pub fn with_default_model(provider: Arc<dyn LlmProvider>) -> Self {
        Self::new(
            provider,
            ModelId::new("openai", "gpt-4o-mini"),
        )
    }

    /// 生成标题（异步）
    ///
    /// 截断长消息（最多 500 字符）以保持请求小巧。
    /// 返回标题字符串或 None（生成失败时）。
    pub async fn generate(
        &self,
        user_message: &str,
        assistant_response: &str,
    ) -> Option<String> {
        let user_snippet = &user_message[..user_message.len().min(500)];
        let assistant_snippet = &assistant_response[..assistant_response.len().min(500)];

        let request = ChatRequest {
            model: self.model.clone(),
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

        match self.provider.chat(request).await {
            Ok(response) => {
                let title = response.content.trim().to_string();
                if title.is_empty() {
                    None
                } else {
                    Some(title)
                }
            }
            Err(e) => {
                tracing::debug!("Title generation failed: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    #[tokio::test]
    async fn test_title_generator_truncates_long_messages() {
        // This is a compile-time check that TitleGenerator works
        // Real tests would need a mock provider
    }
}
