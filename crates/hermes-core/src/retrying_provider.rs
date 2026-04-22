//! 重试包装 Provider 模块
//!
//! 本模块是 `LlmProvider` 的装饰器（Decorator），在原有 Provider 基础上添加了：
//! 1. **自动重试逻辑** — 根据 `RetryPolicy` 对可重试错误进行指数退避重试
//! 2. **凭证池管理** — 通过 `CredentialPool` 实现多凭证的负载均衡和健康检查
//!
//! ## 工作原理
//! - `chat()` 方法从凭证池获取一个健康的凭证，调用底层 Provider
//! - 若调用失败且可重试，则根据 `RetryPolicy` 计算延迟后等待重试
//! - 成功时报告成功到凭证池，失败时报告失败（累计 3 次失败后进入冷却期）
//! - `chat_streaming()` 方法直接透传给底层 Provider（流式调用暂不支持重试）
//!
//! ## 与其他模块的关系
//! - 包装了 `LlmProvider` trait 的具体实现（如 `OpenAiProvider`）
//! - 依赖 `CredentialPool`（`credentials.rs`）进行凭证管理和负载均衡
//! - 依赖 `RetryPolicy`（`retry.rs`）计算重试延迟
//! - 被 `lib.rs` 重新导出为 `RetryingProvider`

use crate::{ChatRequest, ChatResponse, CredentialPool, LlmProvider, ProviderError, RetryPolicy};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// A decorator that wraps an LlmProvider with retry logic and credential pool.
pub struct RetryingProvider {
    inner: Arc<dyn LlmProvider>,
    pool: Arc<CredentialPool>,
    policy: RetryPolicy,
}

impl RetryingProvider {
    pub fn new(
        inner: Arc<dyn LlmProvider>,
        pool: Arc<CredentialPool>,
        policy: RetryPolicy,
    ) -> Self {
        Self {
            inner,
            pool,
            policy,
        }
    }

    async fn call_with_retry(
        &self,
        request: ChatRequest,
        credential_name: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let mut attempt = 0u32;
        loop {
            let result = self.inner.chat(request.clone()).await;

            match result {
                Ok(response) => {
                    self.pool.report_success(credential_name);
                    return Ok(response);
                }
                Err(err) => {
                    if !err.is_retryable() || attempt >= self.policy.max_retries {
                        self.pool.report_failure(credential_name);
                        return Err(err);
                    }

                    let delay = if let Some(retry_after) = err.retry_after_secs() {
                        Duration::from_secs(retry_after)
                    } else {
                        self.policy.delay(attempt)
                    };

                    tracing::warn!(
                        "Provider error (attempt {}/{}), retrying in {:?}: {:?}",
                        attempt + 1,
                        self.policy.max_retries,
                        delay,
                        err
                    );

                    // 检查凭证是否被限流
                    if matches!(err, ProviderError::RateLimit(_)) {
                        if let ProviderError::RateLimit(secs) = err {
                            self.pool.report_rate_limit(credential_name, secs);
                        }
                    }

                    sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl LlmProvider for RetryingProvider {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn supported_models(&self) -> Vec<crate::ModelId> {
        self.inner.supported_models()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let (name, _key) = self
            .pool
            .get()
            .ok_or_else(|| ProviderError::Api("No healthy credentials available".into()))?;

        self.call_with_retry(request, &name).await
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: crate::StreamingCallback,
    ) -> Result<ChatResponse, ProviderError> {
        self.inner.chat_streaming(request, callback).await
    }

    fn estimate_tokens(&self, text: &str, model: &crate::ModelId) -> usize {
        self.inner.estimate_tokens(text, model)
    }

    fn context_length(&self, model: &crate::ModelId) -> Option<usize> {
        self.inner.context_length(model)
    }
}
