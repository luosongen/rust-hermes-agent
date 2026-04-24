//! Retry Utils — 抖动指数退避重试机制
//!
//! 防止并发重试的惊群效应。

use crate::{AgentError, ErrorClassifier};
use std::time::Duration;

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter_ratio: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 12,
            base_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(120),
            jitter_ratio: 0.5,
        }
    }
}

/// 计算抖动指数退避延迟
pub fn jittered_backoff(attempt: u32, config: &RetryConfig) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let delay = if exponent >= 63 || config.base_delay.as_secs_f64() <= 0.0 {
        config.max_delay
    } else {
        let exponential = config.base_delay.as_secs_f64() * (2_u32.pow(exponent) as f64);
        let capped = exponential.min(config.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    };

    // 添加抖动
    let jitter = rand::random::<f64>() * config.jitter_ratio * delay.as_secs_f64();
    delay + Duration::from_secs_f64(jitter)
}

/// 带重试的异步操作
pub async fn with_retry<F, Fut, T>(
    operation: F,
    config: &RetryConfig,
    classifier: &ErrorClassifier,
) -> Result<T, AgentError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, AgentError>>,
{
    let mut last_error = None;

    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(error) => {
                let classified = classifier.classify(&error);

                if !classified.retryable || attempt >= config.max_attempts {
                    return Err(error);
                }

                let delay = jittered_backoff(attempt, config);
                tokio::time::sleep(delay).await;

                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| AgentError::Internal("Retry exhausted".to_string())))
}
