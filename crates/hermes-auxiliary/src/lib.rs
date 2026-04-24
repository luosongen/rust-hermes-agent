//! Hermes Auxiliary Client
//!
//! 多 Provider 解析链 + 自动故障转移的统一 LLM 调用入口。

pub mod adapters;
pub mod client_cache;
pub mod fallback;
pub mod resolver;

use hermes_core::{ChatRequest, ChatResponse, ProviderError};

pub use client_cache::ClientCache;

/// Auxiliary 客户端配置
#[derive(Debug, Clone)]
pub struct AuxiliaryConfig {
    /// 默认 provider
    pub default_provider: String,
    /// 默认 model（可选）
    pub default_model: Option<String>,
    /// 超时时间（秒）
    pub timeout_secs: f64,
}

impl Default for AuxiliaryConfig {
    fn default() -> Self {
        Self {
            default_provider: "openrouter".to_string(),
            default_model: None,
            timeout_secs: 30.0,
        }
    }
}

/// 调用 LLM — 统一入口
pub async fn call_llm(
    _request: ChatRequest,
    _config: &AuxiliaryConfig,
) -> Result<ChatResponse, ProviderError> {
    // Placeholder — will be wired in later tasks
    Err(ProviderError::Api("not yet implemented".into()))
}
