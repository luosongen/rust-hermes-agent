use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;

// =============================================================================
// OpenAI API 提供者实现
// =============================================================================
//
// 该模块实现了 [`crate::traits::LlmProvider`] trait，提供了与 OpenAI Chat API 的交互能力。
//
// ## 请求转换
//
// [`OpenAiProvider::convert_request`] 负责将通用的 `ChatRequest` 转换为 OpenAI API 的请求格式：
// - 将 `Role` 枚举映射为 OpenAI 的角色字符串（"system"、"user"、"assistant"、"tool"）
// - 将 `Content` 类型映射为消息内容（支持文本、图片 URL、工具结果）
// - 将系统提示词作为首条 system 消息插入
// - 将工具定义转换为 OpenAI 的 function calling 格式
//
// ## 响应转换
//
// [`OpenAiProvider::convert_response`] 负责将 OpenAI API 的响应转换为通用的 `ChatResponse`：
// - 提取消息内容
// - 解析 finish_reason（如 "stop"、"length"、"content_filter"）
// - 将工具调用（tool_calls）转换为统一的 `ToolCall` 结构
// - 提取 token 使用量统计
//
// ## 错误处理
//
// 常见的 HTTP 状态码会被转换为对应的 `ProviderError`：
// - `401` → `ProviderError::Auth`（认证失败）
// - `429` → `ProviderError::RateLimit`（限流，60秒后重试）
// - 其他错误码 → `ProviderError::Api`（包含状态码和响应体）
//
// ## 支持的模型
//
// - `openai/gpt-4o` - 128K 上下文窗口
// - `openai/gpt-4-turbo` - 128K 上下文窗口
// - `openai/gpt-4` - 8K 上下文窗口
// - `openai/gpt-3.5-turbo` - 16K 上下文窗口
//
// ## 流式输出
//
// [`chat_streaming`] 方法当前返回 `Err(ProviderError::Api("Streaming not yet implemented"))`，
// 表示流式输出功能尚未实现。

// =============================================================================
// OpenAI API 请求/响应类型定义
// =============================================================================

/// OpenAI Chat Completions API 请求结构
#[derive(Serialize)]
struct OpenAiRequest {
    /// 模型标识符（如 "gpt-4o"）
    model: String,
    /// 消息列表
    messages: Vec<OpenAiMessage>,
    /// 可选的工具定义列表（OpenAI function calling）
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    /// 可选的温度参数（控制随机性）
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    /// 可选的最大 token 数限制
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    /// 是否启用流式输出
    stream: bool,
}

/// OpenAI 消息结构（与 Role 对应：system/user/assistant/tool）
#[derive(Serialize)]
struct OpenAiMessage {
    /// 角色（"system"、"user"、"assistant"、"tool"）
    role: String,
    /// 消息内容（文本字符串或内容块数组，支持多模态）
    content: serde_json::Value,
}

/// OpenAI 工具定义结构
#[derive(Serialize)]
struct OpenAiTool {
    /// 工具类型，固定为 "function"
    #[serde(rename = "type")]
    tool_type: String,
    /// 函数定义
    function: OpenAiFunction,
}

/// OpenAI 函数定义
#[derive(Serialize)]
struct OpenAiFunction {
    /// 函数名称
    name: String,
    /// 函数描述
    description: String,
    /// 函数参数 JSON Schema
    parameters: serde_json::Value,
}

/// OpenAI API 响应结构
#[derive(Deserialize, Debug)]
struct OpenAiResponse {
    /// 响应选项列表（通常取第一个）
    choices: Vec<OpenAiChoice>,
    /// Token 使用量统计
    usage: Option<OpenAiUsage>,
}

/// OpenAI 响应选项
#[derive(Deserialize, Debug)]
struct OpenAiChoice {
    /// 响应消息
    message: OpenAiChoiceMessage,
    /// 结束原因（"stop"、"length"、"content_filter"）
    finish_reason: Option<String>,
}

/// OpenAI 响应消息内容
#[derive(Deserialize, Debug)]
struct OpenAiChoiceMessage {
    /// 角色（通常为 "assistant"）
    role: String,
    /// 消息文本内容
    content: String,
    /// 工具调用列表（如有）
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

/// OpenAI 工具调用请求
#[derive(Deserialize, Debug)]
struct OpenAiToolCall {
    /// 工具调用 ID
    id: String,
    /// 被调用的函数
    #[serde(rename = "function")]
    function: OpenAiFunctionCall,
}

/// OpenAI 函数调用详情
#[derive(Deserialize, Debug)]
struct OpenAiFunctionCall {
    /// 要调用的函数名称
    name: String,
    /// 函数参数（JSON 字符串，需要额外解析）
    arguments: String,
}

/// OpenAI Token 使用量统计
#[derive(Deserialize, Debug)]
struct OpenAiUsage {
    /// 输入 token 数
    prompt_tokens: usize,
    /// 输出 token 数
    completion_tokens: usize,
    /// 总 token 数
    total_tokens: usize,
}

// =============================================================================
// OpenAiProvider 定义
// =============================================================================

/// OpenAI API 提供者
///
/// 使用 OpenAI Chat Completions API 与 LLM 交互。
/// 支持函数调用（function calling）、系统提示词、温度和 token 限制等参数。
///
/// # 示例
///
/// ```
/// use hermes_provider::OpenAiProvider;
///
/// let provider = OpenAiProvider::new("sk-...", None);
/// ```
pub struct OpenAiProvider {
    /// HTTP 客户端
    client: Client,
    /// API 基础地址（默认 https://api.openai.com/v1）
    base_url: String,
    /// API 密钥
    api_key: String,
}

impl OpenAiProvider {
    /// 创建新的 OpenAI Provider 实例
    ///
    /// - `api_key`：OpenAI API 密钥
    /// - `base_url`：可选的自定义 API 基础地址（如需使用代理）
    pub fn new(api_key: impl Into<String>, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            api_key: api_key.into(),
        }
    }

    /// 将通用的 ChatRequest 转换为 OpenAI API 请求格式
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
                    Content::Text(t) => serde_json::json!([{ "type": "text", "text": t }]),
                    Content::Image { url, detail } => {
                        let mut obj = serde_json::json!({ "type": "image_url", "image_url": { "url": url } });
                        if let Some(d) = detail {
                            obj["image_url"]["detail"] = serde_json::json!(d);
                        }
                        serde_json::json!([obj])
                    }
                    Content::ToolResult { content, .. } => serde_json::json!([{ "type": "text", "text": content }]),
                };

                OpenAiMessage { role, content }
            })
            .collect();

        // Prepend system prompt as a system message if present
        let mut all_messages = Vec::new();
        if let Some(sys) = &request.system_prompt {
            all_messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: serde_json::json!([{ "type": "text", "text": sys }]),
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

    /// 将 OpenAI API 响应转换为通用的 ChatResponse
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
