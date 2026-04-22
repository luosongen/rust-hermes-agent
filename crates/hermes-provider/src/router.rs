use std::collections::HashMap;
use std::sync::Arc;

use hermes_core::{ChatRequest, ChatResponse, LlmProvider, ModelId, ProviderError, StreamingCallback};

// =============================================================================
// Provider Router
// =============================================================================
//
// 根据 model.provider 自动选择对应的 Provider
// 支持智能模型路由，包括：
// 1. 基于模型可用性的路由
// 2. 基于上下文长度的路由
// 3. 基于请求类型的路由

/// Provider 路由器
///
/// 根据 ModelId.provider 自动路由到对应的 Provider
pub struct ProviderRouter {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
}

impl Default for ProviderRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRouter {
    /// 创建新的路由器
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// 注册一个 Provider
    pub fn register<P: LlmProvider + 'static>(&mut self, provider: P) {
        let name = provider.name().to_string();
        self.providers.insert(name, Arc::new(provider));
    }

    /// 获取所有已注册的 provider 名称
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// 检查是否支持某个 provider
    pub fn supports(&self, provider: &str) -> bool {
        self.providers.contains_key(provider)
    }

    /// 路由聊天请求到对应的 Provider
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let provider = self
            .providers
            .get(&request.model.provider)
            .ok_or_else(|| {
                ProviderError::Api(format!(
                    "Unknown provider: {}. Available providers: {:?}",
                    request.model.provider,
                    self.provider_names()
                ))
            })?;
        provider.chat(request).await
    }

    /// 流式路由聊天请求到对应的 Provider
    pub async fn chat_streaming(
        &self, 
        request: ChatRequest, 
        callback: StreamingCallback
    ) -> Result<ChatResponse, ProviderError> {
        let provider = self
            .providers
            .get(&request.model.provider)
            .ok_or_else(|| {
                ProviderError::Api(format!(
                    "Unknown provider: {}. Available providers: {:?}",
                    request.model.provider,
                    self.provider_names()
                ))
            })?;
        provider.chat_streaming(request, callback).await
    }

    /// 智能路由 - 根据请求内容和模型可用性选择最佳 Provider
    pub async fn smart_route(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        // 1. 首先尝试使用请求指定的模型
        if self.supports(&request.model.provider) {
            let provider = self.providers.get(&request.model.provider).unwrap();
            let supported_models = provider.supported_models();
            if supported_models.iter().any(|m| m.model == request.model.model) {
                return provider.chat(request).await;
            }
        }

        // 2. 如果指定的模型不可用，根据请求特性选择合适的替代方案
        // 检查请求中的工具调用需求
        let has_tools = request.tools.is_some();
        
        // 检查请求的上下文长度
        let context_length = self.estimate_context_length(&request);
        
        // 优先选择支持工具调用且上下文长度足够的模型
        for (provider_name, provider) in &self.providers {
            let supported_models = provider.supported_models();
            for model in supported_models {
                // 检查模型是否支持工具调用
                let supports_tools = self.supports_tools(provider_name);
                // 检查模型上下文长度是否足够
                let model_context = provider.context_length(&model).unwrap_or(4096);
                
                if (!has_tools || supports_tools) && model_context >= context_length {
                    let mut adjusted_request = request.clone();
                    adjusted_request.model = model;
                    return provider.chat(adjusted_request).await;
                }
            }
        }

        // 3. 如果没有找到合适的模型，返回错误
        Err(ProviderError::Api("No suitable model found for the request".to_string()))
    }

    /// 估算请求的上下文长度
    fn estimate_context_length(&self, request: &ChatRequest) -> usize {
        // 简单估算：计算所有消息的总长度
        let mut total_length = 0;
        for message in &request.messages {
            if let Some(content) = message.content.as_text() {
                total_length += content.len();
            }
        }
        // 保守估算，假设每个字符平均占用1.5个token
        (total_length as f64 * 1.5) as usize
    }

    /// 检查提供者是否支持工具调用
    fn supports_tools(&self, provider_name: &str) -> bool {
        // 目前大多数主流提供者都支持工具调用
        // 这里可以根据实际情况进行更详细的判断
        match provider_name {
            "openai" | "anthropic" | "kimi" | "deepseek" | "glm" | "qwen" => true,
            _ => false,
        }
    }

    /// 获取 Provider
    pub fn get(&self, name: &str) -> Option<&Arc<dyn LlmProvider>> {
        self.providers.get(name)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports() {
        // Just test the basic functionality without a full mock
        let router = ProviderRouter::new();
        assert!(!router.supports("test"));
        assert!(router.provider_names().is_empty());
    }

    #[test]
    fn test_new_router() {
        let router = ProviderRouter::new();
        assert!(router.provider_names().is_empty());
    }

    #[test]
    fn test_supports_tools() {
        let router = ProviderRouter::new();
        assert!(router.supports_tools("openai"));
        assert!(router.supports_tools("anthropic"));
        assert!(!router.supports_tools("unknown"));
    }
}

