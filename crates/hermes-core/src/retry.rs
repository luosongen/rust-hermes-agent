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
            let jitter = rand::random::<u64>() % capped.max(1);
            Duration::from_millis(jitter)
        } else {
            let half = capped / 2;
            let extra = rand::random::<u64>() % half.max(1);
            Duration::from_millis(half + extra)
        }
    }
}
