//! LLM Provider 抽象接口模块
//!
//! 本模块定义了 `LlmProvider` trait，是所有 LLM 后端（如 OpenAI、Anthropic 等）的统一接口。
//!
//! ## 主要类型
//! - **LlmProvider**（trait）: 异步的 LLM 调用接口，包含以下核心方法:
//!   - `name()` — 提供者名称（如 "openai", "anthropic"）
//!   - `supported_models()` — 该提供者支持的模型列表
//!   - `chat()` — 非流式聊天完成
//!   - `chat_streaming()` — 流式聊天完成
//!   - `estimate_tokens()` — 估算 token 数量
//!   - `context_length()` — 获取模型的上下文窗口大小
//!
//! ## 与其他模块的关系
//! - 由 `hermes-provider` 中的 `OpenAiProvider` 实现
//! - 被 `agent.rs` 使用来进行 LLM 调用
//! - 被 `retrying_provider.rs` 包装以添加重试逻辑
//! - `ChatRequest`/`ChatResponse` 类型定义在 `types.rs` 中
//! - `ProviderError` 定义在 `error.rs` 中

use crate::{ChatRequest, ChatResponse, ModelId, ProviderError, StreamingCallback};
use async_trait::async_trait;

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
