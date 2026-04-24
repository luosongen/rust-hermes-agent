//! Integration tests for hermes-auxiliary

use hermes_auxiliary::{AuxiliaryConfig, ClientCache, ProviderResolver, call_llm, fallback};
use hermes_core::{ChatRequest, Message, ModelId, ProviderError};

#[test]
fn test_auxiliary_config_defaults() {
    let config = AuxiliaryConfig::default();
    assert_eq!(config.default_provider, "openrouter");
    assert_eq!(config.timeout_secs, 30.0);
    assert!(config.default_model.is_none());
}

#[test]
fn test_client_cache_new_is_empty() {
    let cache = ClientCache::new();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_provider_resolver_created() {
    let _resolver = ProviderResolver::new();
    // Smoke test — resolver was created without panic
}

#[test]
fn test_fallback_should_retry_rate_limit() {
    assert!(fallback::should_retry(&ProviderError::RateLimit(5), Some("openai")));
}

#[tokio::test]
async fn test_call_llm_with_invalid_provider() {
    let config = AuxiliaryConfig::default();
    let request = ChatRequest {
        model: ModelId::new("nonexistent", "model"),
        messages: vec![Message::user("hello")],
        tools: None,
        system_prompt: None,
        temperature: None,
        max_tokens: None,
    };

    let result = call_llm(request, &config).await;
    assert!(result.is_err());
}
