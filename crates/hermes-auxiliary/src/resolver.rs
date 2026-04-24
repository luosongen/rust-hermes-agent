//! Provider resolver — 多 Provider 解析链
//!
//! 按优先级尝试 provider：OpenRouter → Anthropic → 自定义端点 → 按 API key 遍历

use std::sync::Arc;

use hermes_core::{LlmProvider, ProviderError};
use hermes_provider::{
    AnthropicProvider, DeepSeekProvider, GlmProvider, KimiProvider, OpenAiProvider,
    OpenRouterProvider,
};

use crate::adapters::{ClientAdapter, GenericAdapter};
use crate::AuxiliaryConfig;

/// Provider 解析链优先级
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStep {
    /// OpenRouter 聚合网关
    OpenRouter,
    /// 原生 Anthropic API
    Anthropic,
    /// 自定义端点
    CustomEndpoint,
    /// 按 API key 遍历其他 provider
    ApiKeyProviders,
}

/// Provider 解析结果
pub type ResolvedClient = Box<dyn ClientAdapter>;

/// Provider 解析器
pub struct ProviderResolver {
    /// 已解析的 provider 步骤顺序
    steps: Vec<ProviderStep>,
}

impl ProviderResolver {
    /// 创建默认解析器（按优先级链）
    pub fn new() -> Self {
        Self {
            steps: vec![
                ProviderStep::OpenRouter,
                ProviderStep::Anthropic,
                ProviderStep::CustomEndpoint,
                ProviderStep::ApiKeyProviders,
            ],
        }
    }

    /// 按优先级解析 provider，返回第一个可用的客户端
    pub async fn resolve(
        &self,
        provider: Option<&str>,
        _model: Option<&str>,
        config: &AuxiliaryConfig,
    ) -> Result<(ResolvedClient, String), ProviderError> {
        // 如果显式指定了 provider，直接使用
        if let Some(provider_name) = provider {
            return self.resolve_specific(provider_name, config).await;
        }

        // 否则按解析链依次尝试
        for step in &self.steps {
            match step {
                ProviderStep::OpenRouter => {
                    if let Ok(client) = self.try_openrouter().await {
                        return Ok((Box::new(client), "openrouter".into()));
                    }
                }
                ProviderStep::Anthropic => {
                    if let Ok(client) = self.try_anthropic().await {
                        return Ok((Box::new(client), "anthropic".into()));
                    }
                }
                ProviderStep::CustomEndpoint => {
                    if let Ok(client) = self.try_custom_endpoint(config).await {
                        return Ok((Box::new(client), config.default_provider.clone()));
                    }
                }
                ProviderStep::ApiKeyProviders => {
                    if let Ok((client, name)) = self.try_api_key_providers().await {
                        return Ok((client, name));
                    }
                }
            }
        }

        Err(ProviderError::Api("no available provider found".into()))
    }

    /// 解析指定的 provider
    async fn resolve_specific(
        &self,
        provider: &str,
        config: &AuxiliaryConfig,
    ) -> Result<(ResolvedClient, String), ProviderError> {
        match provider {
            "openrouter" => {
                let client = self.try_openrouter().await?;
                Ok((Box::new(client), "openrouter".into()))
            }
            "anthropic" => {
                let client = self.try_anthropic().await?;
                Ok((Box::new(client), "anthropic".into()))
            }
            "openai" => {
                let client = self.try_openai().await?;
                Ok((Box::new(client), "openai".into()))
            }
            _ => {
                // 尝试通过自定义端点
                if let Ok(client) = self.try_custom_endpoint(config).await {
                    Ok((Box::new(client), provider.into()))
                } else {
                    // 尝试作为 API key provider
                    self.try_api_key_providers().await
                }
            }
        }
    }

    /// 尝试 OpenRouter
    async fn try_openrouter(&self) -> Result<GenericAdapter, ProviderError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENROUTER_API_KEY"));

        match api_key {
            Ok(key) if !key.is_empty() => {
                let provider: Arc<dyn LlmProvider> = Arc::new(OpenRouterProvider::new(key));
                Ok(GenericAdapter::new(provider))
            }
            _ => Err(ProviderError::Auth),
        }
    }

    /// 尝试 Anthropic
    async fn try_anthropic(&self) -> Result<GenericAdapter, ProviderError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .or_else(|_| std::env::var("HERMES_ANTHROPIC_API_KEY"));

        match api_key {
            Ok(key) if !key.is_empty() => {
                let provider: Arc<dyn LlmProvider> = Arc::new(AnthropicProvider::new(key));
                Ok(GenericAdapter::new(provider))
            }
            _ => Err(ProviderError::Auth),
        }
    }

    /// 尝试 OpenAI
    async fn try_openai(&self) -> Result<GenericAdapter, ProviderError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"));

        match api_key {
            Ok(key) if !key.is_empty() => {
                let base_url = std::env::var("OPENAI_BASE_URL").ok();
                let provider: Arc<dyn LlmProvider> = Arc::new(OpenAiProvider::new(key, base_url));
                Ok(GenericAdapter::new(provider))
            }
            _ => Err(ProviderError::Auth),
        }
    }

    /// 尝试自定义端点
    async fn try_custom_endpoint(
        &self,
        _config: &AuxiliaryConfig,
    ) -> Result<GenericAdapter, ProviderError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))
            .unwrap_or_default();

        if !api_key.is_empty() {
            let base_url = std::env::var("OPENAI_BASE_URL").ok();
            let provider: Arc<dyn LlmProvider> = Arc::new(OpenAiProvider::new(api_key, base_url));
            Ok(GenericAdapter::new(provider))
        } else {
            Err(ProviderError::Auth)
        }
    }

    /// 遍历 API key provider
    async fn try_api_key_providers(&self) -> Result<(ResolvedClient, String), ProviderError> {
        // 尝试 GLM
        if let Ok(key) = std::env::var("GLM_API_KEY") {
            if !key.is_empty() {
                let provider: Arc<dyn LlmProvider> = Arc::new(GlmProvider::new(key));
                return Ok((Box::new(GenericAdapter::new(provider)), "glm".into()));
            }
        }

        // 尝试 DeepSeek
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.is_empty() {
                let provider: Arc<dyn LlmProvider> = Arc::new(DeepSeekProvider::new(key));
                return Ok((Box::new(GenericAdapter::new(provider)), "deepseek".into()));
            }
        }

        // 尝试 Kimi
        if let Ok(key) = std::env::var("KIMI_API_KEY") {
            if !key.is_empty() {
                let provider: Arc<dyn LlmProvider> = Arc::new(KimiProvider::new(key));
                return Ok((Box::new(GenericAdapter::new(provider)), "kimi".into()));
            }
        }

        Err(ProviderError::Auth)
    }
}

impl Default for ProviderResolver {
    fn default() -> Self {
        Self::new()
    }
}
