use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;

// =============================================================================
// OpenRouter API Provider
// =============================================================================
//
// API: https://openrouter.ai/docs
// OpenRouter is OpenAI-compatible and supports 200+ models
//
// Authentication: API Key in Authorization header
// Base URL: https://openrouter.ai/api/v1

// =============================================================================
// Request/Response Types
// =============================================================================

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    stream: bool,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<OpenRouterChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

// OpenAI-compatible types
#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "function")]
    function: OpenAiFunctionCall,
}

#[derive(Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

// =============================================================================
// OpenRouterProvider
// =============================================================================

pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
}

impl OpenRouterProvider {
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

fn convert_messages(messages: &[hermes_core::Message]) -> Vec<OpenAiMessage> {
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

            OpenAiMessage { role, content }
        })
        .collect()
}

fn convert_tool_calls(
    tool_calls: Option<Vec<OpenAiToolCall>>,
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

#[allow(dead_code)]

// =============================================================================
// LlmProvider impl
// =============================================================================

#[async_trait]
impl hermes_core::LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("openrouter", "openai/gpt-4o"),
            ModelId::new("openrouter", "openai/gpt-4-turbo"),
            ModelId::new("openrouter", "anthropic/claude-3.5-sonnet"),
            ModelId::new("openrouter", "google/gemini-pro-1.5"),
            ModelId::new("openrouter", "meta-llama/llama-3-70b-instruct"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = "https://openrouter.ai/api/v1/chat/completions";

        let req = convert_request(request);

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://rust-hermes-agent")
            .header("X-Title", "Rust Hermes Agent")
            .json(&req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
        }

        let or_response: OpenRouterResponse = response.json().await?;
        convert_response(or_response)
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
        // Return context window size based on model
        match model.model.as_str() {
            m if m.contains("gpt-4o") || m.contains("gpt-4-turbo") => Some(128_000),
            m if m.contains("claude-3.5") => Some(200_000),
            m if m.contains("gemini") => Some(128_000),
            m if m.contains("llama-3-70b") => Some(128_000),
            _ => Some(128_000),
        }
    }
}

fn convert_request(request: ChatRequest) -> OpenRouterRequest {
    let messages = convert_messages(&request.messages);

    // Handle system prompt
    let mut all_messages = Vec::new();
    if let Some(sys) = request.system_prompt {
        all_messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    all_messages.extend(messages);

    OpenRouterRequest {
        model: request.model.model.clone(),
        messages: all_messages,
        tools: request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| OpenAiTool {
                    tool_type: "function".to_string(),
                    function: OpenAiFunction {
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

fn convert_response(response: OpenRouterResponse) -> Result<ChatResponse, ProviderError> {
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
        let provider = OpenRouterProvider::new("test-key");
        let models = provider.supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.provider == "openrouter"));
    }

    #[test]
    fn test_context_length() {
        let provider = OpenRouterProvider::new("test-key");
        let model = ModelId::new("openrouter", "openai/gpt-4o");
        assert_eq!(provider.context_length(&model), Some(128_000));
    }
}