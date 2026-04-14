use crate::credentials::CredentialPool;

#[test]
fn test_credential_pool_add_and_get() {
    let pool = CredentialPool::new();
    pool.add("key1", "sk-test1".to_string());
    pool.add("key2", "sk-test2".to_string());

    let names = pool.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"key1".into()));
    assert!(names.contains(&"key2".into()));

    let (name, key) = pool.get().expect("should get a key");
    assert!(["key1", "key2"].contains(&name.as_str()));
    assert!(["sk-test1", "sk-test2"].contains(&key.as_str()));
}

#[test]
fn test_credential_pool_failure_tracking() {
    let pool = CredentialPool::new();
    pool.add("good", "sk-good".to_string());
    pool.add("bad", "sk-bad".to_string());

    for _ in 0..3 {
        pool.report_failure("bad");
    }

    let health = pool.health();
    let bad_health = health.iter().find(|h| h.name == "bad").unwrap();
    assert!(!bad_health.available);

    let (name, _) = pool.get().expect("should get good key");
    assert_eq!(name, "good");
}

#[test]
fn test_credential_pool_rate_limit_reports_correctly() {
    let pool = CredentialPool::new();
    pool.add("key1", "sk-key1".to_string());

    pool.report_rate_limit("key1", 30);

    let health = pool.health();
    let key1_health = health.iter().find(|h| h.name == "key1").unwrap();
    assert!(!key1_health.available);
    assert!(key1_health.cooldown_until.is_some());
}

#[test]
fn test_credential_pool_success_clears_failures() {
    let pool = CredentialPool::new();
    pool.add("key1", "sk-key1".to_string());

    pool.report_failure("key1");
    pool.report_failure("key1");
    pool.report_failure("key1");

    pool.report_success("key1");

    let health = pool.health();
    let key1_health = health.iter().find(|h| h.name == "key1").unwrap();
    assert_eq!(key1_health.failures, 0);
}
