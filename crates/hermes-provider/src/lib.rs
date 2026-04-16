//! hermes-provider crate
//!
//! 提供大语言模型（LLM）提供者实现。
//!
//! 该 crate 目前包含 OpenAI API 提供者的实现，负责：
//! - 将通用的 `ChatRequest` 转换为各平台特定的 API 请求格式
//! - 将各平台的响应转换为统一的 `ChatResponse`
//! - 处理 API 认证、限流等错误
//!
//! 模块结构：
//! - [`traits`] - 从 hermes-core 重导出的 `LlmProvider` trait 定义
//! - [`openai`] - OpenAI API 提供者 [`OpenAiProvider`] 的具体实现
//!
//! # 示例
//!
//! ```ignore
//! use hermes_provider::OpenAiProvider;
//!
//! let provider = OpenAiProvider::new("your-api-key", None);
//! ```
//!
//! [`OpenAiProvider`]: openai::OpenAiProvider

/// 从 hermes-core 重导出的 LlmProvider trait
pub mod traits;
/// OpenAI API 提供者实现
pub mod openai;
/// Anthropic Claude API 提供者实现
pub mod anthropic;
/// OpenRouter 聚合 API 提供者实现（支持 200+ 模型）
pub mod openrouter;
/// 智谱 AI (GLM) 提供者实现
pub mod glm;
/// MiniMax 海螺 AI 提供者实现
pub mod minimax;
/// Kimi (Moonshot AI) 提供者实现
pub mod kimi;
/// DeepSeek 提供者实现
pub mod deepseek;
/// 阿里云百炼 (Qwen) 提供者实现
pub mod qwen;
/// Provider 路由器（根据 model.provider 自动选择 Provider）
pub mod router;

pub use traits::*;
pub use openai::OpenAiProvider;
pub use anthropic::AnthropicProvider;
pub use openrouter::OpenRouterProvider;
pub use glm::GlmProvider;
pub use minimax::MiniMaxProvider;
pub use kimi::KimiProvider;
pub use deepseek::DeepSeekProvider;
pub use qwen::QwenProvider;
pub use router::ProviderRouter;
