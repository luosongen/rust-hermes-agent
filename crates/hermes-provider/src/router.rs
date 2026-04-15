use std::collections::HashMap;
use std::sync::Arc;

use hermes_core::{ChatRequest, ChatResponse, LlmProvider, ProviderError};

// =============================================================================
// Provider Router
// =============================================================================
//
// 根据 model.provider 自动选择对应的 Provider

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
}
