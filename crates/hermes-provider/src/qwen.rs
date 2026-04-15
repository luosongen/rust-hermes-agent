use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;

// =============================================================================
// Qwen (阿里云百炼) Provider
// =============================================================================
//
// API: https://dashscope.aliyuncs.com/api/v1
// Qwen is OpenAI-compatible via 阿里云百炼服务
//
// Authentication: API Key in Authorization header (Bearer prefix)
// Base URL: https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Serialize)]
struct QwenRequest {
    model: String,
    input: QwenInput,
    parameters: QwenParameters,
}

#[derive(Serialize)]
struct QwenInput {
    messages: Vec<QwenMessage>,
}

#[derive(Serialize)]
struct QwenMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct QwenParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_format: Option<QwenResultFormat>,
}

#[derive(Serialize)]
struct QwenResultFormat {
    message: String,
}

#[derive(Deserialize)]
struct QwenResponse {
    output: QwenOutput,
    usage: Option<QwenUsage>,
    #[allow(dead_code)]
    request_id: String,
}

#[derive(Deserialize)]
struct QwenOutput {
    choices: Vec<QwenChoice>,
}

#[derive(Deserialize)]
struct QwenChoice {
    finish_reason: Option<String>,
    message: QwenResponseMessage,
}

#[derive(Deserialize)]
struct QwenResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct QwenUsage {
    input_tokens: usize,
    output_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

// =============================================================================
// QwenProvider
// =============================================================================

pub struct QwenProvider {
    client: Client,
    api_key: String,
}

impl QwenProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn convert_messages(messages: &[hermes_core::Message]) -> Vec<QwenMessage> {
    use hermes_core::{Content, Role};

    messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            }
            .to_string();

            let content = match &m.content {
                Content::Text(t) => t.clone(),
                Content::Image { url, .. } => url.clone(),
                Content::ToolResult { content, .. } => content.clone(),
            };

            QwenMessage { role, content }
        })
        .collect()
}

// =============================================================================
// LlmProvider impl
// =============================================================================

#[async_trait]
impl hermes_core::LlmProvider for QwenProvider {
    fn name(&self) -> &str {
        "qwen"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("qwen", "qwen-turbo"),
            ModelId::new("qwen", "qwen-plus"),
            ModelId::new("qwen", "qwen-max"),
            ModelId::new("qwen", "qwen-long"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = "https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation";

        let req = convert_request(request);

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
        }

        let qwen_response: QwenResponse = response.json().await?;
        convert_response(qwen_response)
    }

    async fn chat_streaming(
        &self,
        _request: ChatRequest,
        _callback: hermes_core::StreamingCallback,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::Api("Streaming not yet implemented".into()))
    }

    fn estimate_tokens(&self, text: &str, _model: &ModelId) -> usize {
        text.len() / 4
    }

    fn context_length(&self, model: &ModelId) -> Option<usize> {
        match model.model.as_str() {
            m if m.contains("qwen-turbo") => Some(32_000),
            m if m.contains("qwen-plus") => Some(128_000),
            m if m.contains("qwen-max") => Some(8_000),
            m if m.contains("qwen-long") => Some(30_000),
            _ => Some(128_000),
        }
    }
}

fn convert_request(request: ChatRequest) -> QwenRequest {
    let messages = convert_messages(&request.messages);

    // Handle system prompt - Qwen uses a special format
    let all_messages = if request.system_prompt.is_some() {
        let mut msgs = vec![QwenMessage {
            role: "system".to_string(),
            content: request.system_prompt.unwrap_or_default(),
        }];
        msgs.extend(messages);
        msgs
    } else {
        messages
    };

    QwenRequest {
        model: request.model.model.clone(),
        input: QwenInput { messages: all_messages },
        parameters: QwenParameters {
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            result_format: Some(QwenResultFormat {
                message: "message".to_string(),
            }),
        },
    }
}

fn convert_response(response: QwenResponse) -> Result<ChatResponse, ProviderError> {
    let choice = response
        .output
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Api("No choices in response".into()))?;

    let finish_reason = match choice.finish_reason.as_deref() {
        Some("stop") => hermes_core::FinishReason::Stop,
        Some("length") => hermes_core::FinishReason::Length,
        _ => hermes_core::FinishReason::Other,
    };

    Ok(ChatResponse {
        content: choice.message.content,
        finish_reason,
        tool_calls: None,
        reasoning: None,
        usage: response.usage.map(|u| hermes_core::Usage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        }),
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_core::LlmProvider;

    #[test]
    fn test_supported_models() {
        let provider = QwenProvider::new("test-key");
        let models = provider.supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.provider == "qwen"));
    }

    #[test]
    fn test_context_length() {
        let provider = QwenProvider::new("test-key");
        let model = ModelId::new("qwen", "qwen-plus");
        assert_eq!(provider.context_length(&model), Some(128_000));

        let model = ModelId::new("qwen", "qwen-turbo");
        assert_eq!(provider.context_length(&model), Some(32_000));
    }
}
