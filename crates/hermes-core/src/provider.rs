use async_trait::async_trait;
use crate::{ChatRequest, ChatResponse, ModelId, ProviderError, StreamingCallback};

/// Trait implemented by all LLM provider backends.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Provider name (e.g. "openai", "anthropic")
    fn name(&self) -> &str;

    /// List of models this provider supports
    fn supported_models(&self) -> Vec<ModelId>;

    /// Non-streaming chat completion
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;

    /// Streaming chat completion
    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: StreamingCallback,
    ) -> Result<ChatResponse, ProviderError>;

    /// Rough token count estimate
    fn estimate_tokens(&self, text: &str, model: &ModelId) -> usize;

    /// Context window for the given model
    fn context_length(&self, model: &ModelId) -> Option<usize>;
}
