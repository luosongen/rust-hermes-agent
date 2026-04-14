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
