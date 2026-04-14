use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;

// =============================================================================
// OpenAI API Request/Response Types
// =============================================================================

#[derive(Serialize)]
struct OpenAiRequest {
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

#[derive(Deserialize, Debug)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize, Debug)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenAiChoiceMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize, Debug)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "function")]
    function: OpenAiFunctionCall,
}

#[derive(Deserialize, Debug)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize, Debug)]
struct OpenAiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

// =============================================================================
// OpenAiProvider
// =============================================================================

pub struct OpenAiProvider {
    client: Client,
    base_url: String,
    api_key: String,
}

impl OpenAiProvider {
    pub fn new(api_key: impl Into<String>, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            api_key: api_key.into(),
        }
    }

    fn convert_request(&self, request: ChatRequest) -> OpenAiRequest {
        use hermes_core::{Content, Role};

        let messages: Vec<OpenAiMessage> = request
            .messages
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
            .collect();

        // Prepend system prompt as a system message if present
        let mut all_messages = Vec::new();
        if let Some(sys) = &request.system_prompt {
            all_messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: sys.clone(),
            });
        }
        all_messages.extend(messages);

        let tools = request.tools.map(|tools| {
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
        });

        OpenAiRequest {
            model: request.model.model.clone(),
            messages: all_messages,
            tools,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: false,
        }
    }

    fn convert_response(&self, response: OpenAiResponse) -> Result<hermes_core::ChatResponse, ProviderError> {
        use hermes_core::{ChatResponse, FinishReason, ToolCall, Usage};
        use std::collections::HashMap;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::Api("No choices in response".into()))?;

        let finish_reason = match choice.finish_reason.as_deref() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Other,
        };

        let tool_calls = choice.message.tool_calls.map(|tcs| {
            tcs.into_iter()
                .map(|tc| {
                    let arguments: HashMap<String, serde_json::Value> =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    ToolCall {
                        id: tc.id,
                        name: tc.function.name,
                        arguments,
                    }
                })
                .collect()
        });

        let usage = response.usage.map(|u| Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        });

        Ok(ChatResponse {
            content: choice.message.content,
            finish_reason,
            tool_calls,
            reasoning: None,
            usage,
        })
    }
}

// =============================================================================
// LlmProvider impl
// =============================================================================

#[async_trait]
impl crate::traits::LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("openai", "gpt-4o"),
            ModelId::new("openai", "gpt-4-turbo"),
            ModelId::new("openai", "gpt-4"),
            ModelId::new("openai", "gpt-3.5-turbo"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);
        let oai_request = self.convert_request(request);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&oai_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 401 {
                return Err(ProviderError::Auth);
            }
            if status.as_u16() == 429 {
                return Err(ProviderError::RateLimit(60));
            }
            return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
        }

        let oai_response: OpenAiResponse = response.json().await?;
        self.convert_response(oai_response)
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
            "gpt-4o" | "gpt-4-turbo" => Some(128_000),
            "gpt-4" => Some(8_192),
            "gpt-3.5-turbo" => Some(16_385),
            _ => None,
        }
    }
}
