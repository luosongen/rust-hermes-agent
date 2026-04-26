//! Integration tests for core infrastructure modules
//!
//! Tests credential_pool, error_classifier, and retry_utils modules.

use hermes_core::{
    AgentError, classify_api_error, classify_http_error, ClassifiedError, CredentialPool,
    CredentialEntry, CredentialStatus, FailoverReason, jittered_backoff, PoolStrategy,
    ProviderError, RetryConfig, with_retry,
};
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================================================
    // Credential Pool Tests (using CredPool from credential_pool module)
    // =============================================================================

    #[test]
    fn test_credential_pool_round_robin() {
        // Test that RoundRobin strategy selects credentials evenly (by use_count)
        let pool = CredentialPool::new(PoolStrategy::RoundRobin);
        pool.add("openai", "key1");
        pool.add("openai", "key2");

        // Select should return key1 first (use_count=0 for both, but min_by_key picks first)
        let first = pool.select("openai").unwrap();
        assert_eq!(first, "key1");

        // Second select should return key2 since key1 now has higher use_count
        let second = pool.select("openai").unwrap();
        assert_eq!(second, "key2");
    }

    #[test]
    fn test_credential_pool_mark_exhausted() {
        let pool = CredentialPool::new(PoolStrategy::RoundRobin);
        pool.add("openai", "key1");
        pool.add("openai", "key2");

        // Select and mark first as exhausted
        let first = pool.select("openai").unwrap();
        assert_eq!(first, "key1");
        pool.mark_exhausted("openai", "key1");

        // Next select should get key2 since key1 is exhausted
        let second = pool.select("openai").unwrap();
        assert_eq!(second, "key2");

        // Mark key2 exhausted too
        pool.mark_exhausted("openai", "key2");

        // No credentials available
        assert!(pool.select("openai").is_none());
    }

    #[test]
    fn test_credential_pool_random_strategy() {
        let pool = CredentialPool::new(PoolStrategy::Random);
        pool.add("openai", "key1");
        pool.add("openai", "key2");
        pool.add("openai", "key3");

        // All three should be selectable
        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            if let Some(key) = pool.select("openai") {
                seen.insert(key);
            }
        }

        // Should have seen all three keys at some point
        assert!(seen.contains("key1"));
        assert!(seen.contains("key2"));
        assert!(seen.contains("key3"));
    }

    #[test]
    fn test_credential_pool_fill_first_strategy() {
        let pool = CredentialPool::new(PoolStrategy::FillFirst);
        pool.add("openai", "key1");
        pool.add("openai", "key2");

        // FillFirst always returns first available credential
        for _ in 0..10 {
            let selected = pool.select("openai").unwrap();
            assert_eq!(selected, "key1");
        }
    }

    #[test]
    fn test_credential_pool_least_used_strategy() {
        let pool = CredentialPool::new(PoolStrategy::LeastUsed);
        pool.add("openai", "key1");
        pool.add("openai", "key2");

        // First select key1
        let first = pool.select("openai").unwrap();
        assert_eq!(first, "key1");

        // Second select should pick key2 (lower use_count)
        let second = pool.select("openai").unwrap();
        assert_eq!(second, "key2");

        // Third select should pick key1 again (lower use_count now)
        let third = pool.select("openai").unwrap();
        assert_eq!(third, "key1");
    }

    #[test]
    fn test_credential_pool_unknown_provider() {
        let pool = CredentialPool::new(PoolStrategy::RoundRobin);
        pool.add("openai", "key1");

        // No credentials for unknown provider
        assert!(pool.select("unknown").is_none());
    }

    // =============================================================================
    // Error Classifier Tests
    // =============================================================================

    #[test]
    fn test_classify_auth_error() {
        let result = classify_api_error(
            &ProviderError::Auth,
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Auth);
        assert!(!result.retryable);
        assert!(!result.should_fallback);
    }

    #[test]
    fn test_classify_rate_limit() {
        let result = classify_api_error(
            &ProviderError::RateLimit(30),
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert!(result.retryable);
        assert!(!result.should_fallback); // 30s < 60s threshold
    }

    #[test]
    fn test_classify_rate_limit_long() {
        let result = classify_api_error(
            &ProviderError::RateLimit(120),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert!(result.should_fallback); // 120s > 60s threshold
    }

    #[test]
    fn test_classify_context_overflow() {
        let result = classify_api_error(
            &ProviderError::ContextTooLarge,
            Some("anthropic"),
            None,
            200000,
            200000,
        );
        assert_eq!(result.reason, FailoverReason::ContextOverflow);
        assert!(result.should_compress);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_invalid_model() {
        let result = classify_api_error(
            &ProviderError::InvalidModel("gpt-5".into()),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::ModelNotFound);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_api_error_billing_message() {
        let result = classify_api_error(
            &ProviderError::Api("insufficient credits balance".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Billing);
        assert!(result.should_fallback);
    }

    #[test]
    fn test_classify_api_error_rate_limit_message() {
        let result = classify_api_error(
            &ProviderError::Api("rate limit exceeded, too many requests".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert!(result.retryable);
    }

    #[test]
    fn test_classify_http_error_429() {
        let result = classify_http_error(
            429,
            "Too many requests",
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert_eq!(result.status_code, Some(429));
        assert!(result.retryable);
    }

    #[test]
    fn test_classify_http_error_401() {
        let result = classify_http_error(
            401,
            "Unauthorized",
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Auth);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_http_error_503() {
        let result = classify_http_error(
            503,
            "Service unavailable",
            Some("openai"),
            None,
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Overloaded);
        assert!(result.retryable);
        assert!(result.should_fallback);
    }

    #[test]
    fn test_classified_error_is_auth() {
        let auth = ClassifiedError {
            reason: FailoverReason::Auth,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(auth.is_auth());

        let auth_perm = ClassifiedError {
            reason: FailoverReason::AuthPermanent,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(auth_perm.is_auth());

        let billing = ClassifiedError {
            reason: FailoverReason::Billing,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(!billing.is_auth());
    }

    // =============================================================================
    // Retry Utils Tests
    // =============================================================================

    #[test]
    fn test_jittered_backoff_increases() {
        let config = RetryConfig {
            max_attempts: 12,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            jitter_ratio: 0.0, // Disable jitter for predictable testing
        };

        // Backoff should increase with attempts
        let delay1 = jittered_backoff(1, &config);
        let delay2 = jittered_backoff(2, &config);
        let delay3 = jittered_backoff(3, &config);

        // Each successive delay should be greater than or equal to the previous
        assert!(delay2 >= delay1);
        assert!(delay3 >= delay2);
    }

    #[test]
    fn test_jittered_backoff_max_delay() {
        let config = RetryConfig {
            max_attempts: 12,
            base_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(10),
            jitter_ratio: 0.0,
        };

        // High attempts should cap at max_delay
        let delay = jittered_backoff(100, &config);
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn test_jittered_backoff_with_jitter() {
        let config = RetryConfig {
            max_attempts: 12,
            base_delay: Duration::from_secs(10),
            max_delay: Duration::from_secs(100),
            jitter_ratio: 0.5, // 50% jitter
        };

        let delay1 = jittered_backoff(2, &config);
        let delay2 = jittered_backoff(2, &config);

        // With jitter, consecutive calls should differ
        // (statistically very likely to differ)
        let delay_base = Duration::from_secs(20); // 10 * 2^1 = 20s

        // Delays should be within [base_delay, base_delay * (1 + jitter_ratio)]
        // For attempt 2: base = 10 * 2 = 20s, with 50% jitter: [20, 30]
        assert!(delay1 >= delay_base);
        assert!(delay1 <= Duration::from_secs_f64(30.0));
    }

    #[tokio::test]
    async fn test_with_retry_success_first_try() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            jitter_ratio: 0.0,
        };

        let result = with_retry(
            || async { Ok::<_, AgentError>(42) },
            &config,
            None,
            None,
            100,
            128000,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_retry_eventually_succeeds() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            jitter_ratio: 0.0,
        };

        let attempt_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = with_retry(
            move || {
                let count = attempt_count_clone.clone();
                async move {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if count.load(std::sync::atomic::Ordering::SeqCst) < 2 {
                        Err(AgentError::Provider(ProviderError::RateLimit(1)))
                    } else {
                        Ok(42)
                    }
                }
            },
            &config,
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_with_retry_exhausted() {
        let config = RetryConfig {
            max_attempts: 2,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            jitter_ratio: 0.0,
        };

        let result = with_retry(
            || async { Err::<i32, _>(AgentError::Provider(ProviderError::Auth)) },
            &config,
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        )
        .await;

        assert!(result.is_err());
    }
}
