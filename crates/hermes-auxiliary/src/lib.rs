//! Hermes Auxiliary Client
//!
//! 多 Provider 解析链 + 自动故障转移的统一 LLM 调用入口。

pub mod adapters;
pub mod client_cache;
pub mod fallback;
pub mod resolver;

use hermes_core::{ChatRequest, ChatResponse, ProviderError};

pub use client_cache::ClientCache;
pub use resolver::{ProviderResolver, ProviderStep, ResolvedClient};

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
///
/// 自动解析 provider、处理故障转移。
pub async fn call_llm(
    request: ChatRequest,
    config: &AuxiliaryConfig,
) -> Result<ChatResponse, ProviderError> {
    let resolver = ProviderResolver::new();

    let model_id = request.model.clone();
    let provider_name = model_id.provider.clone();

    // Step 1: Resolve provider
    let (client, resolved_provider) = resolver
        .resolve(Some(&provider_name), Some(&model_id.model), config)
        .await
        .map_err(|e| {
            tracing::error!("failed to resolve provider '{}': {}", provider_name, e);
            e
        })?;

    // Step 2: Make the call
    match client.chat(request.clone()).await {
        Ok(response) => Ok(response),
        Err(error) => {
            // Step 3: Check if we should fallback
            if fallback::should_fallback(&error, Some(&resolved_provider)) {
                tracing::warn!(
                    "provider '{}' returned fallback-eligible error: {}",
                    resolved_provider, error
                );
                // Try next provider in chain
                match resolver
                    .resolve(None, Some(&model_id.model), config)
                    .await
                {
                    Ok((fallback_client, _fallback_name)) => {
                        fallback_client.chat(request).await
                    }
                    Err(_) => Err(error),
                }
            } else {
                Err(error)
            }
        }
    }
}
