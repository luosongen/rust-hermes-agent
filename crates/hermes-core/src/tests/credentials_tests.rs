use crate::credential_pool::{CredentialPool, PoolStrategy};

#[test]
fn test_credential_pool_add_and_get() {
    let pool = CredentialPool::new(PoolStrategy::RoundRobin);
    pool.add("provider1", "key1", "sk-test1");
    pool.add("provider1", "key2", "sk-test2");

    let names = pool.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"key1".into()));
    assert!(names.contains(&"key2".into()));

    let (name, key) = pool.get("provider1").expect("should get a key");
    assert!(["key1", "key2"].contains(&name.as_str()));
    assert!(["sk-test1", "sk-test2"].contains(&key.as_str()));
}

#[test]
fn test_credential_pool_failure_tracking() {
    let pool = CredentialPool::new(PoolStrategy::RoundRobin);
    pool.add("provider1", "good", "sk-good");
    pool.add("provider1", "bad", "sk-bad");

    for _ in 0..3 {
        pool.report_failure("provider1", "bad");
    }

    let health = pool.health();
    let bad_health = health.iter().find(|h| h.name == "bad").unwrap();
    assert!(!bad_health.available);

    let (name, _) = pool.get("provider1").expect("should get good key");
    assert_eq!(name, "good");
}

#[test]
fn test_credential_pool_rate_limit_reports_correctly() {
    let pool = CredentialPool::new(PoolStrategy::RoundRobin);
    pool.add("provider1", "key1", "sk-key1");

    pool.report_rate_limit("provider1", "key1", 30);

    let health = pool.health();
    let key1_health = health.iter().find(|h| h.name == "key1").unwrap();
    assert!(!key1_health.available);
    assert!(key1_health.cooldown_until.is_some());
}

#[test]
fn test_credential_pool_success_clears_failures() {
    let pool = CredentialPool::new(PoolStrategy::RoundRobin);
    pool.add("provider1", "key1", "sk-key1");

    pool.report_failure("provider1", "key1");
    pool.report_failure("provider1", "key1");
    pool.report_failure("provider1", "key1");

    pool.report_success("provider1", "key1");

    let health = pool.health();
    let key1_health = health.iter().find(|h| h.name == "key1").unwrap();
    assert_eq!(key1_health.failures, 0);
}
