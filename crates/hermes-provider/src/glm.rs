use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;

// =============================================================================
// GLM (智谱 AI) Provider
// =============================================================================
//
// API: https://open.bigmodel.cn/api/paas/v4
// GLM is OpenAI-compatible
//
// Authentication: API Key in Authorization header (Bearer prefix)
// Base URL: https://open.bigmodel.cn/api/paas/v4/chat/completions

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Serialize)]
struct GlmRequest {
    model: String,
    messages: Vec<GlmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GlmTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    stream: bool,
}

#[derive(Serialize)]
struct GlmMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct GlmResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<GlmChoice>,
    usage: Option<GlmUsage>,
}

#[derive(Deserialize)]
struct GlmChoice {
    message: GlmResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct GlmResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<GlmToolCall>>,
}

#[derive(Deserialize)]
struct GlmUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

#[derive(Serialize)]
struct GlmTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: GlmFunction,
}

#[derive(Serialize)]
struct GlmFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct GlmToolCall {
    id: String,
    #[serde(rename = "function")]
    function: GlmFunctionCall,
}

#[derive(Deserialize)]
struct GlmFunctionCall {
    name: String,
    arguments: String,
}

// =============================================================================
// GlmProvider
// =============================================================================

pub struct GlmProvider {
    client: Client,
    api_key: String,
}

impl GlmProvider {
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

fn convert_messages(messages: &[hermes_core::Message]) -> Vec<GlmMessage> {
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

            GlmMessage { role, content }
        })
        .collect()
}

fn convert_tool_calls(
    tool_calls: Option<Vec<GlmToolCall>>,
) -> Option<Vec<hermes_core::ToolCall>> {
    tool_calls.map(|tcs| {
        tcs.into_iter()
            .map(|tc| {
                let arguments: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                hermes_core::ToolCall {
                    id: tc.id,
                    name: tc.function.name,
                    arguments,
                }
            })
            .collect()
    })
}

// =============================================================================
// LlmProvider impl
// =============================================================================

#[async_trait]
impl hermes_core::LlmProvider for GlmProvider {
    fn name(&self) -> &str {
        "glm"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("glm", "glm-4-plus"),
            ModelId::new("glm", "glm-4"),
            ModelId::new("glm", "glm-4-flash"),
            ModelId::new("glm", "glm-3-turbo"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = "https://open.bigmodel.cn/api/paas/v4/chat/completions";

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

        let glm_response: GlmResponse = response.json().await?;
        convert_response(glm_response)
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
            m if m.contains("glm-4") => Some(128_000),
            m if m.contains("glm-3") => Some(32_000),
            _ => Some(128_000),
        }
    }
}

fn convert_request(request: ChatRequest) -> GlmRequest {
    let messages = convert_messages(&request.messages);

    // Handle system prompt
    let mut all_messages = Vec::new();
    if let Some(sys) = request.system_prompt {
        all_messages.push(GlmMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    all_messages.extend(messages);

    GlmRequest {
        model: request.model.model.clone(),
        messages: all_messages,
        tools: request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| GlmTool {
                    tool_type: "function".to_string(),
                    function: GlmFunction {
                        name: t.name,
                        description: t.description,
                        parameters: t.parameters,
                    },
                })
                .collect()
        }),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        stream: false,
    }
}

fn convert_response(response: GlmResponse) -> Result<ChatResponse, ProviderError> {
    let choice = response
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
        tool_calls: convert_tool_calls(choice.message.tool_calls),
        reasoning: None,
        usage: response.usage.map(|u| hermes_core::Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
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
        let provider = GlmProvider::new("test-key");
        let models = provider.supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.provider == "glm"));
    }

    #[test]
    fn test_context_length() {
        let provider = GlmProvider::new("test-key");
        let model = ModelId::new("glm", "glm-4-plus");
        assert_eq!(provider.context_length(&model), Some(128_000));

        let model = ModelId::new("glm", "glm-3-turbo");
        assert_eq!(provider.context_length(&model), Some(32_000));
    }
}
