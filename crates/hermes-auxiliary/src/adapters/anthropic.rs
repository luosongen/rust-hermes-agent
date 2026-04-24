//! Anthropic adapter — 包装 AnthropicProvider 为 ClientAdapter

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, LlmProvider, ModelId, ProviderError};
use hermes_provider::AnthropicProvider;

use super::ClientAdapter;

pub struct AnthropicAdapter {
    provider: AnthropicProvider,
}

impl AnthropicAdapter {
    pub fn new(provider: AnthropicProvider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ClientAdapter for AnthropicAdapter {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        self.provider.supported_models()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.provider.chat(request).await
    }
}
