use crate::retry::RetryPolicy;
use std::time::Duration;

#[test]
fn test_retry_policy_default_sane() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.max_retries, 3);
    assert_eq!(policy.base_delay, Duration::from_millis(500));
    assert_eq!(policy.max_delay, Duration::from_secs(30));
}

#[test]
fn test_retry_policy_exponential_growth() {
    let policy = RetryPolicy::new(5, Duration::from_millis(100), Duration::from_secs(60));
    for i in 0..5 {
        let delay = policy.delay(i);
        assert!(delay > Duration::ZERO);
        assert!(delay <= Duration::from_secs(60));
    }
}

#[test]
fn test_retry_policy_max_delay_cap() {
    let policy = RetryPolicy::new(10, Duration::from_millis(100), Duration::from_millis(500));
    for i in 0..10 {
        let delay = policy.delay(i);
        assert!(delay <= Duration::from_millis(500));
    }
}
