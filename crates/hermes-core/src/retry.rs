//! 重试策略模块
//!
//! 本模块定义了 LLM Provider 调用失败时的重试策略，采用指数退避（exponential backoff）算法。
//!
//! ## 主要类型
//! - **RetryPolicy**: 重试策略配置结构体
//!   - `max_retries` — 最大重试次数（不包含初始调用）
//!   - `base_delay` — 首次重试的基础延迟
//!   - `max_delay` — 延迟上限
//!   - `full_jitter` — 是否使用完全随机抖动
//!
//! ## 重试延迟计算
//! 采用指数退避 + 抖动的算法：
//! - 延迟 = min(base_delay * 2^attempt, max_delay)
//! - 若启用完整抖动：延迟为 `[0, 上限]` 范围内的随机值
//! - 若启用半抖动：延迟为 `[上限/2, 上限]` 范围内的随机值
//!
//! ## 与其他模块的关系
//! - 被 `retrying_provider.rs` 使用来计算重试等待时间
//! - 配置通过 `Config::load()` 从配置/环境变量加载

use std::time::Duration;

/// Configuration for retry behavior with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (not including the initial call).
    pub max_retries: u32,
    /// Base delay before first retry.
    pub base_delay: Duration,
    /// Maximum delay cap.
    pub max_delay: Duration,
    /// Enable full jitter (randomize entire delay).
    pub full_jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            full_jitter: true,
        }
    }
}

impl RetryPolicy {
    pub fn new(max_retries: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
            max_delay,
            full_jitter: true,
        }
    }

    /// Calculate the delay for a given attempt number (0-indexed).
    pub fn delay(&self, attempt: u32) -> Duration {
        let exp = 2u32.saturating_pow(attempt);
        let delay_ms = self.base_delay.as_millis() * exp as u128;
        let capped = delay_ms.min(self.max_delay.as_millis()) as u64;

        if self.full_jitter {
            let jitter_max = capped.max(1);
            let jitter = 1 + rand::random::<u64>() % (jitter_max - 1);
            Duration::from_millis(jitter.max(1))
        } else {
            let half = capped / 2;
            let extra = rand::random::<u64>() % half.max(1);
            Duration::from_millis((half + extra).max(1))
        }
    }
}
