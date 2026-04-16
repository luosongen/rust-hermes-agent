use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::collections::HashMap;

// =============================================================================
// OpenRouter 聚合 API Provider 实现
// =============================================================================
//
// 该模块实现了 [`crate::traits::LlmProvider`] trait，提供了与 OpenRouter 聚合 API 的交互能力。
//
// OpenRouter 是一个统一的 API 网关，支持访问 200+ 第三方 LLM 模型。
//
// ## API 特性
// - API 文档：https://openrouter.ai/docs
// - 认证方式：API Key 放在 Authorization header 中
// - 基础地址：https://openrouter.ai/api/v1
// - 兼容性：OpenRouter 采用 OpenAI 兼容的 API 格式
// - 特殊 Header：`HTTP-Referer` 和 `X-Title` 用于 OpenRouter 排行统计
//
// ## 请求转换
//
// [`convert_request`] 负责将通用的 `ChatRequest` 转换为 OpenRouter API 的请求格式：
// - 将 `Role` 枚举映射为角色字符串
// - 将 `Content` 类型映射为消息内容
// - 将系统提示词作为首条 system 消息插入
// - 将工具定义转换为 OpenAI 的 function calling 格式
//
// ## 响应转换
//
// [`convert_response`] 负责将 OpenRouter API 的响应转换为通用的 `ChatResponse`：
// - 提取消息内容
// - 解析 finish_reason
// - 将工具调用转换为统一的 `ToolCall` 结构
// - 提取 token 使用量统计
//
// ## 支持的模型（示例）
//
// OpenRouter 支持大量模型，以下是预配置的部分示例：
// - `openrouter/openai/gpt-4o` - 128K 上下文窗口
// - `openrouter/anthropic/claude-3.5-sonnet` - 200K 上下文窗口
// - `openrouter/google/gemini-pro-1.5` - 128K 上下文窗口
// - `openrouter/meta-llama/llama-3-70b-instruct` - 128K 上下文窗口
//
// 实际支持的完整模型列表请参阅 OpenRouter 官方文档。
//
// ## 流式输出
//
// [`chat_streaming`] 方法当前返回 `Err(ProviderError::Api("Streaming not yet implemented"))`，
// 表示流式输出功能尚未实现。

// =============================================================================
// 请求/响应类型定义
// =============================================================================

/// OpenRouter API 请求结构
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

/// OpenRouter API 响应结构
#[derive(Deserialize)]
struct OpenRouterResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<OpenRouterChoice>,
    usage: Option<Usage>,
}

/// OpenRouter 响应选项
#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
    finish_reason: Option<String>,
}

/// OpenRouter 响应消息内容
#[derive(Deserialize)]
struct OpenRouterMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

/// OpenRouter Token 使用量统计
#[derive(Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

/// OpenAI 兼容消息结构（OpenRouter 使用）
#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

/// OpenAI 兼容工具定义（OpenRouter 使用）
#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

/// OpenAI 兼容函数定义
#[derive(Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// OpenAI 兼容工具调用
#[derive(Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "function")]
    function: OpenAiFunctionCall,
}

/// OpenAI 兼容函数调用详情
#[derive(Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

// =============================================================================
// OpenRouterProvider 定义
// =============================================================================

/// OpenRouter 聚合 API 提供者
///
/// 通过 OpenRouter 统一网关访问 200+ 第三方 LLM 模型。
/// OpenRouter API 与 OpenAI 兼容，支持函数调用等功能。
pub struct OpenRouterProvider {
    /// HTTP 客户端
    client: Client,
    /// API 密钥
    api_key: String,
}

impl OpenRouterProvider {
    /// 创建新的 OpenRouter Provider 实例
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
        }
    }
}

// =============================================================================
// 辅助函数
// =============================================================================

/// 将 hermes-core 的 Message 列表转换为 OpenRouter API 的消息格式
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