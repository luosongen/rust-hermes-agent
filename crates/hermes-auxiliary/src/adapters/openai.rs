//! OpenAI adapter — 直通适配（OpenAI 兼容协议）

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, LlmProvider, ModelId, ProviderError};
use hermes_provider::OpenAiProvider;

use super::ClientAdapter;

pub struct OpenAiAdapter {
    provider: OpenAiProvider,
}

impl OpenAiAdapter {
    pub fn new(provider: OpenAiProvider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ClientAdapter for OpenAiAdapter {
    fn provider_name(&self) -> &str {
        "openai"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        self.provider.supported_models()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.provider.chat(request).await
    }
}
