use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;

// =============================================================================
// MiniMax Provider
// =============================================================================
//
// API: https://api.minimax.chat/v1
// MiniMax is OpenAI-compatible
//
// Authentication: API Key in Authorization header (Bearer prefix)
// Base URL: https://api.minimax.chat/v1/chat/completions

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Serialize)]
struct MiniMaxRequest {
    model: String,
    messages: Vec<MiniMaxMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<MiniMaxTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    stream: bool,
}

#[derive(Serialize)]
struct MiniMaxMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MiniMaxResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<MiniMaxChoice>,
    usage: Option<MiniMaxUsage>,
}

#[derive(Deserialize)]
struct MiniMaxChoice {
    message: MiniMaxResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct MiniMaxResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<MiniMaxToolCall>>,
}

#[derive(Deserialize)]
struct MiniMaxUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

#[derive(Serialize)]
struct MiniMaxTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: MiniMaxFunction,
}

#[derive(Serialize)]
struct MiniMaxFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct MiniMaxToolCall {
    id: String,
    #[serde(rename = "function")]
    function: MiniMaxFunctionCall,
}

#[derive(Deserialize)]
struct MiniMaxFunctionCall {
    name: String,
    arguments: String,
}

// =============================================================================
// MiniMaxProvider
// =============================================================================

pub struct MiniMaxProvider {
    client: Client,
    api_key: String,
}

impl MiniMaxProvider {
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

fn convert_messages(messages: &[hermes_core::Message]) -> Vec<MiniMaxMessage> {
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

            MiniMaxMessage { role, content }
        })
        .collect()
}

fn convert_tool_calls(
    tool_calls: Option<Vec<MiniMaxToolCall>>,
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
impl hermes_core::LlmProvider for MiniMaxProvider {
    fn name(&self) -> &str {
        "minimax"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("minimax", "MiniMax-Text-01"),
            ModelId::new("minimax", "hailuo-02"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = "https://api.minimax.chat/v1/chat/completions";

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

        let minimax_response: MiniMaxResponse = response.json().await?;
        convert_response(minimax_response)
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
            m if m.contains("MiniMax-Text-01") => Some(1_000_000),
            m if m.contains("hailuo-02") => Some(128_000),
            _ => Some(128_000),
        }
    }
}

fn convert_request(request: ChatRequest) -> MiniMaxRequest {
    let messages = convert_messages(&request.messages);

    // Handle system prompt
    let mut all_messages = Vec::new();
    if let Some(sys) = request.system_prompt {
        all_messages.push(MiniMaxMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    all_messages.extend(messages);

    MiniMaxRequest {
        model: request.model.model.clone(),
        messages: all_messages,
        tools: request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| MiniMaxTool {
                    tool_type: "function".to_string(),
                    function: MiniMaxFunction {
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

fn convert_response(response: MiniMaxResponse) -> Result<ChatResponse, ProviderError> {
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
        let provider = MiniMaxProvider::new("test-key");
        let models = provider.supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.provider == "minimax"));
    }

    #[test]
    fn test_context_length() {
        let provider = MiniMaxProvider::new("test-key");
        let model = ModelId::new("minimax", "MiniMax-Text-01");
        assert_eq!(provider.context_length(&model), Some(1_000_000));

        let model = ModelId::new("minimax", "hailuo-02");
        assert_eq!(provider.context_length(&model), Some(128_000));
    }
}
