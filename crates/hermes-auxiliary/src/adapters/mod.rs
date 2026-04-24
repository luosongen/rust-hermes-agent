//! Client adapter trait — 统一的 LLM 客户端接口

pub mod openai;
pub mod anthropic;

pub use openai::OpenAiAdapter;
pub use anthropic::AnthropicAdapter;

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};

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
