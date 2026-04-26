use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use futures_util::StreamExt;
use reqwest::Client;

// =============================================================================
// Anthropic API 提供者实现
// =============================================================================
//
// 该模块实现了 [`crate::traits::LlmProvider`] trait，提供了与 Anthropic Claude API 的交互能力。
//
// ## Anthropic Messages API vs OpenAI Chat Completions
//
// | OpenAI | Anthropic |
// |--------|-----------|
// | `/v1/chat/completions` | `POST /v1/messages` |
// | `role: system/user/assistant` | `role: user/assistant` + `system` 字段 |
// | `Content` 字符串 | `ContentBlock` 数组 |
// | `tool_calls` | `tool_use` |
// | 响应在 `.choices[0].message` | 响应在 `.content[0].text` |
//
// ## 请求转换
//
// [`AnthropicProvider::convert_request`] 负责将通用的 `ChatRequest` 转换为 Anthropic API 格式：
// - 系统提示词作为独立的 `system` 字段（不是消息）
// - 消息角色只支持 `user` 和 `assistant`（Anthropic 不支持 system role）
// - 内容转换为 `ContentBlock` 数组格式
// - 工具定义转换为 Anthropic 的 tools 格式
//
// ## 响应转换
//
// [`AnthropicProvider::convert_response`] 负责将 Anthropic API 响应转换为通用格式：
// - 提取 `.content[0].text` 作为响应内容
// - 解析 `stop_reason`（end_turn/max_tokensstop_sequence）
// - 将工具调用（`tool_use`）转换为统一的 `ToolCall` 结构
// - 提取 token 使用量（包括 cache 相关的 tokens）
//
// ## 支持的模型
//
// - `anthropic/claude-3-5-sonnet-20241022` - 200K 上下文窗口
// - `anthropic/claude-3-5-haiku-20241022` - 200K 上下文窗口
// - `anthropic/claude-3-opus-20240229` - 200K 上下文窗口
// - `anthropic/claude-3-haiku-20240307` - 200K 上下文窗口
// - `anthropic/claude-4-sonnet-20250514` - 200K 上下文窗口
// - `anthropic/claude-4-opus-20250514` - 200K 上下文窗口
//
// ## 流式输出
//
// [`chat_streaming`] 方法使用 Anthropic 的 Server-Sent Events (SSE) 流式响应格式。

// =============================================================================
// Anthropic API Request/Response Types
// =============================================================================

/// Anthropic Messages API 请求
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    stream: bool,
}

/// Anthropic 消息 - role 只支持 user 和 assistant
#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContentBlock>,
}

/// Anthropic 内容块
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
}

/// Anthropic 工具定义
#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

/// Anthropic API 响应
#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    id: String,
    type_: String,
    role: String,
    content: Vec<AnthropicResponseContent>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

/// Anthropic 响应内容块
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicResponseContent {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
}

/// Anthropic 使用量统计
#[derive(Deserialize, Debug)]
struct AnthropicUsage {
    input_tokens: usize,
    output_tokens: usize,
    cache_creation_tokens: Option<usize>,
    cache_read_tokens: Option<usize>,
}

/// Anthropic 流式事件
#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    index: Option<usize>,
    delta: Option<AnthropicStreamDelta>,
    #[serde(rename = "stop_reason")]
    stop_reason: Option<String>,
}

/// Anthropic 流式 Delta
#[derive(Debug, Deserialize)]
struct AnthropicStreamDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
    #[serde(rename = "partial_json")]
    partial_json: Option<String>,
}

// =============================================================================
// AnthropicProvider
// =============================================================================

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
        }
    }

    fn convert_request(&self, request: ChatRequest) -> AnthropicRequest {
        use hermes_core::{Content, Role};

        // Anthropic 不支持 system role，系统提示词单独处理
        let system = request.system_prompt;

        // 转换消息
        let messages: Vec<AnthropicMessage> = request
            .messages
            .iter()
            .filter(|m| m.role != Role::System) // 过滤掉 system 消息（已单独处理）
            .map(|m| {
                let role = match m.role {
                    Role::System => "user", // 不应该到达这里，但以防万一
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "user", // Anthropic 用 user role 携带工具结果
                };

                let content = match &m.content {
                    Content::Text(text) => vec![AnthropicContentBlock::Text { text: text.clone() }],
                    Content::Image { url, .. } => {
                        // Anthropic 支持 base64 图片，但这里简化处理
                        vec![AnthropicContentBlock::Text { text: format!("[Image: {}]", url) }]
                    }
                    Content::ToolResult { tool_call_id, content } => {
                        // 工具结果作为 text content 传递
                        vec![AnthropicContentBlock::Text {
                            text: format!("[Tool {}: {}]", tool_call_id, content),
                        }]
                    }
                };

                AnthropicMessage { role: role.to_string(), content }
            })
            .collect();

        // 转换工具定义
        let tools = request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| AnthropicTool {
                    name: t.name,
                    description: t.description,
                    input_schema: t.parameters,
                })
                .collect()
        });

        AnthropicRequest {
            model: request.model.model.clone(),
            messages,
            system,
            tools,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: false,
        }
    }

    fn convert_response(&self, response: AnthropicResponse) -> Result<ChatResponse, ProviderError> {
        use hermes_core::{ChatResponse, FinishReason, ToolCall, Usage};

        // 提取文本内容
        let content = response
            .content
            .iter()
            .filter_map(|c| {
                if let AnthropicResponseContent::Text { text } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // 解析结束原因
        let finish_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("stop_sequence") => FinishReason::Stop,
            _ => FinishReason::Other,
        };

        // 提取工具调用
        let tool_calls: Option<Vec<ToolCall>> = {
            let tcs: Vec<ToolCall> = response
                .content
                .iter()
                .filter_map(|c| {
                    if let AnthropicResponseContent::ToolUse { id, name, input } = c {
                        // 将 JSON 对象转换为 HashMap
                        let arguments: std::collections::HashMap<String, serde_json::Value> =
                            serde_json::from_value(input.clone()).unwrap_or_default();
                        Some(ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            if tcs.is_empty() {
                None
            } else {
                Some(tcs)
            }
        };

        // 转换使用量
        let usage = Some(Usage {
            input_tokens: response.usage.input_tokens,
            output_tokens: response.usage.output_tokens,
            cache_read_tokens: response.usage.cache_read_tokens,
            cache_write_tokens: response.usage.cache_creation_tokens,
            reasoning_tokens: None,
        });

        Ok(ChatResponse {
            content,
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
impl crate::traits::LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("anthropic", "claude-3-5-sonnet-20241022"),
            ModelId::new("anthropic", "claude-3-5-haiku-20241022"),
            ModelId::new("anthropic", "claude-3-opus-20240229"),
            ModelId::new("anthropic", "claude-3-haiku-20240307"),
            ModelId::new("anthropic", "claude-4-sonnet-20250514"),
            ModelId::new("anthropic", "claude-4-opus-20250514"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = "https://api.anthropic.com/v1/messages";
        let anthropic_request = self.convert_request(request);

        // Anthropic 需要特定的 headers
        let response = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&anthropic_request)
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

        let anthropic_response: AnthropicResponse = response.json().await?;
        self.convert_response(anthropic_response)
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: hermes_core::StreamingCallback,
    ) -> Result<ChatResponse, ProviderError> {
        let mut anthropic_request = self.convert_request(request);
        anthropic_request.stream = true;

        let mut full_content = String::new();
        let mut full_tool_calls: Vec<hermes_core::ToolCall> = Vec::new();
        let mut stop_reason = "end_turn".to_string();

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&anthropic_request)
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

        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| ProviderError::Network(e))?;
            let text = String::from_utf8_lossy(&bytes);

            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                        match event.event_type.as_str() {
                            "content_block_delta" => {
                                if let Some(delta) = event.delta {
                                    if delta.delta_type == "text_delta" {
                                        if let Some(text) = delta.text {
                                            full_content += &text;
                                            callback(ChatResponse {
                                                content: text.clone(),
                                                finish_reason: hermes_core::FinishReason::Stop,
                                                tool_calls: None,
                                                reasoning: None,
                                                usage: None,
                                            });
                                        }
                                    } else if delta.delta_type == "input_json_delta" {
                                        if let Some(json) = delta.partial_json {
                                            let idx = event.index.unwrap_or(0);
                                            if full_tool_calls.len() <= idx {
                                                full_tool_calls.resize(idx + 1, hermes_core::ToolCall {
                                                    id: format!("tool-{}", idx),
                                                    name: String::new(),
                                                    arguments: std::collections::HashMap::new(),
                                                });
                                            }
                                            let args = &mut full_tool_calls[idx].arguments;
                                            let current: String = args.iter()
                                                .map(|(k,v)| format!("{}: {}", k, v))
                                                .collect::<Vec<_>>()
                                                .join(", ");
                                            args.insert(format!("__partial_{}", idx), serde_json::Value::String(current + &json));
                                        }
                                    }
                                }
                            }
                            "message_delta" => {
                                if let Some(ref sr) = event.stop_reason {
                                    stop_reason = sr.clone();
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let finish_reason = match stop_reason.as_str() {
            "end_turn" => hermes_core::FinishReason::Stop,
            "max_tokens" => hermes_core::FinishReason::Length,
            _ => hermes_core::FinishReason::Other,
        };

        Ok(ChatResponse {
            content: full_content,
            finish_reason,
            tool_calls: if full_tool_calls.is_empty() {
                None
            } else {
                Some(full_tool_calls)
            },
            reasoning: None,
            usage: None,
        })
    }

    fn estimate_tokens(&self, text: &str, _model: &ModelId) -> usize {
        // 粗略估算：中文约 2 字符/token，英文约 4 字符/token
        // Claude 使用 Claude tokenizer，实际应该用专用估算方法
        text.len() / 4
    }

    fn context_length(&self, model: &ModelId) -> Option<usize> {
        match model.model.as_str() {
            // Claude 3.5 和 4 系列都有 200K 上下文
            m if m.contains("claude-3-5") || m.contains("claude-4") => Some(200_000),
            // Claude 3 系列也是 200K
            m if m.contains("claude-3") => Some(200_000),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_core::{ChatRequest, LlmProvider, Message, ModelId};

    #[test]
    fn test_convert_request_with_system_prompt() {
        let provider = AnthropicProvider::new("test-key");
        let request = ChatRequest {
            model: ModelId::new("anthropic", "claude-3-5-sonnet-20241022"),
            messages: vec![Message::user("Hello")],
            tools: None,
            system_prompt: Some("You are helpful".to_string()),
            temperature: None,
            max_tokens: Some(1024),
        };

        let anthropic_req = provider.convert_request(request);
        assert!(anthropic_req.system.is_some());
        assert_eq!(anthropic_req.system.unwrap(), "You are helpful");
        assert_eq!(anthropic_req.messages.len(), 1);
        assert_eq!(anthropic_req.model, "claude-3-5-sonnet-20241022");
    }

    #[test]
    fn test_convert_request_with_tools() {
        let provider = AnthropicProvider::new("test-key");
        let request = ChatRequest {
            model: ModelId::new("anthropic", "claude-3-5-sonnet-20241022"),
            messages: vec![Message::user("Use the calculator")],
            tools: Some(vec![hermes_core::ToolDefinition {
                name: "calculator".to_string(),
                description: "A calculator tool".to_string(),
                parameters: serde_json::json!({"type": "object"}),
            }]),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
        };

        let anthropic_req = provider.convert_request(request);
        assert!(anthropic_req.tools.is_some());
        let tools = anthropic_req.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "calculator");
    }

    #[test]
    fn test_supported_models() {
        let provider = AnthropicProvider::new("test-key");
        let models = provider.supported_models();
        assert!(models.len() >= 6);
        assert!(models.iter().any(|m| m.model.contains("claude-3-5-sonnet")));
        assert!(models.iter().any(|m| m.model.contains("claude-4")));
    }

    #[test]
    fn test_context_length() {
        let provider = AnthropicProvider::new("test-key");
        let model = ModelId::new("anthropic", "claude-3-5-sonnet-20241022");
        assert_eq!(provider.context_length(&model), Some(200_000));

        let model = ModelId::new("anthropic", "claude-4-opus-20250514");
        assert_eq!(provider.context_length(&model), Some(200_000));
    }
}
