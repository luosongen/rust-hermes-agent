//! Fallback logic — 支付/连接错误时自动切换到下一个 provider

use hermes_core::{classify_api_error, ProviderError};

/// 检测是否为需要 fallback 的错误
pub fn should_fallback(error: &ProviderError, provider: Option<&str>) -> bool {
    let classified = classify_api_error(error, provider, None, 0, 200_000);
    classified.should_fallback
}

/// 检测是否为重试错误
pub fn should_retry(error: &ProviderError, provider: Option<&str>) -> bool {
    let classified = classify_api_error(error, provider, None, 0, 200_000);
    classified.retryable
}

/// 检测是否需要压缩上下文
pub fn should_compress(error: &ProviderError, provider: Option<&str>) -> bool {
    let classified = classify_api_error(error, provider, None, 0, 200_000);
    classified.should_compress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_fallback_on_auth() {
        assert!(!should_fallback(&ProviderError::Auth, Some("openai")));
    }

    #[test]
    fn test_should_fallback_on_billing() {
        let err = ProviderError::Api("insufficient credits balance".into());
        assert!(should_fallback(&err, Some("openai")));
    }

    #[test]
    fn test_should_retry_on_rate_limit() {
        assert!(should_retry(&ProviderError::RateLimit(5), Some("openai")));
    }

    #[test]
    fn test_should_compress_on_context_overflow() {
        assert!(should_compress(&ProviderError::ContextTooLarge, Some("anthropic")));
    }
}
