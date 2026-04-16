use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;

// =============================================================================
// 阿里云百炼 (Qwen) Provider 实现
// =============================================================================
//
// 该模块实现了 [`crate::traits::LlmProvider`] trait，提供了与阿里云百炼 Qwen API 的交互能力。
//
// ## API 特性
// - API 文档：https://dashscope.aliyuncs.com/api/v1
// - 认证方式：API Key 放在 Authorization header 中（Bearer 前缀）
// - 基础地址：https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation
// - 兼容性：Qwen API 采用与 OpenAI 不同的请求格式（使用 `input`/`parameters` 结构）
//
// ## 请求转换
//
// [`convert_request`] 负责将通用的 `ChatRequest` 转换为 Qwen API 的请求格式：
// - 将 `Role` 枚举映射为角色字符串（"system"、"user"、"assistant"、"tool"）
// - 将 `Content` 类型映射为消息内容（支持文本、图片 URL、工具结果）
// - 将系统提示词作为首条 system 消息插入
// - 响应格式必须设置为 `"message"` 以返回标准消息格式
// - 注意：Qwen API 当前版本不支持工具调用（tool_calls），该字段在响应中始终为 None
//
// ## 响应转换
//
// [`convert_response`] 负责将 Qwen API 的响应转换为通用的 `ChatResponse`：
// - 提取消息内容
// - 解析 finish_reason（如 "stop"、"length"）
// - 工具调用不支持，返回 None
// - 提取 token 使用量统计
//
// ## 支持的模型
//
// - `qwen/qwen-turbo` - 32K 上下文窗口
// - `qwen/qwen-plus` - 128K 上下文窗口
// - `qwen/qwen-max` - 8K 上下文窗口
// - `qwen/qwen-long` - 30K 上下文窗口（长文本专用）
//
// ## 流式输出
//
// [`chat_streaming`] 方法当前返回 `Err(ProviderError::Api("Streaming not yet implemented"))`，
// 表示流式输出功能尚未实现。

// =============================================================================
// 请求/响应类型定义
// =============================================================================

/// Qwen API 请求结构（阿里云百炼格式）
#[derive(Serialize)]
struct QwenRequest {
    model: String,
    input: QwenInput,
    parameters: QwenParameters,
}

/// Qwen 输入结构
#[derive(Serialize)]
struct QwenInput {
    messages: Vec<QwenMessage>,
}

/// Qwen 消息结构
#[derive(Serialize)]
struct QwenMessage {
    role: String,
    content: String,
}

/// Qwen 请求参数
#[derive(Serialize)]
struct QwenParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_format: Option<QwenResultFormat>,
}

/// Qwen 响应格式配置
#[derive(Serialize)]
struct QwenResultFormat {
    message: String,
}

/// Qwen API 响应结构
#[derive(Deserialize)]
struct QwenResponse {
    output: QwenOutput,
    usage: Option<QwenUsage>,
    #[allow(dead_code)]
    request_id: String,
}

/// Qwen 输出结构
#[derive(Deserialize)]
struct QwenOutput {
    choices: Vec<QwenChoice>,
}

/// Qwen 响应选项
#[derive(Deserialize)]
struct QwenChoice {
    finish_reason: Option<String>,
    message: QwenResponseMessage,
}

/// Qwen 响应消息内容
#[derive(Deserialize)]
struct QwenResponseMessage {
    #[allow(dead_code)]
    role: String,
    content: String,
}

/// Qwen Token 使用量统计
#[derive(Deserialize)]
struct QwenUsage {
    input_tokens: usize,
    output_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

// =============================================================================
// QwenProvider 定义
// =============================================================================

/// 阿里云百炼 (Qwen) API 提供者
///
/// 使用阿里云百炼服务与 Qwen 系列 LLM 交互。
/// Qwen API 与 OpenAI API 格式不同，采用 `input`/`parameters` 结构。
pub struct QwenProvider {
    /// HTTP 客户端
    client: Client,
    /// API 密钥
    api_key: String,
}

impl QwenProvider {
    /// 创建新的 Qwen Provider 实例
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

/// 将 hermes-core 的 Message 列表转换为 Qwen API 的消息格式
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
