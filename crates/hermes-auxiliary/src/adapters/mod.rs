//! Client adapter trait — 统一的 LLM 客户端接口

pub mod openai;
pub mod anthropic;

pub use openai::OpenAiAdapter;
pub use anthropic::AnthropicAdapter;

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, LlmProvider, ModelId, ProviderError};
use std::sync::Arc;

/// 统一的 LLM 客户端适配器
#[async_trait]
pub trait ClientAdapter: Send + Sync {
    /// Provider 名称
    fn provider_name(&self) -> &str;

    /// 支持的模型列表
    fn supported_models(&self) -> Vec<ModelId>;

    /// 非流式 chat completion
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
}

/// 泛型适配器——包装任意 Arc<dyn LlmProvider> 实现 ClientAdapter
pub struct GenericAdapter {
    name: String,
    provider: Arc<dyn LlmProvider>,
}

impl GenericAdapter {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        let name = provider.name().to_string();
        Self { name, provider }
    }
}

#[async_trait]
impl ClientAdapter for GenericAdapter {
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn supported_models(&self) -> Vec<ModelId> {
        self.provider.supported_models()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.provider.chat(request).await
    }
}
