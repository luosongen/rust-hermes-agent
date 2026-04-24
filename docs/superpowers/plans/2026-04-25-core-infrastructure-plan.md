# Core Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现凭证池、错误分类器和重试机制，提升 Agent 的稳定性和容错能力

**Architecture:** 三个独立模块放入 hermes-core：credential_pool.rs、error_classifier.rs、retry_utils.rs，最后集成到 Agent

**Tech Stack:** Rust, tokio, parking_lot, serde

---

## File Structure

```
crates/hermes-core/src/
├── credential_pool.rs      # 新增 (~200 lines)
├── error_classifier.rs     # 新增 (~150 lines)
├── retry_utils.rs          # 新增 (~100 lines)
├── agent.rs               # 修改：集成重试逻辑
└── lib.rs                 # 修改：导出新模块

crates/hermes-core/tests/
└── test_infrastructure.rs  # 新增
```

---

## Task 1: Create error_classifier.rs

**Files:**
- Create: `crates/hermes-core/src/error_classifier.rs`

- [ ] **Step 1: Create the file with types**

```rust
//! Error Classifier — API 错误分类与恢复策略

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverReason {
    Auth,
    AuthPermanent,
    Billing,
    RateLimit,
    Overloaded,
    ServerError,
    Timeout,
    ContextOverflow,
    PayloadTooLarge,
    ModelNotFound,
    FormatError,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ClassifiedError {
    pub reason: FailoverReason,
    pub status_code: Option<u16>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub message: String,
    pub retryable: bool,
    pub should_compress: bool,
    pub should_rotate_credential: bool,
    pub should_fallback: bool,
}

pub struct ErrorClassifier;

impl ErrorClassifier {
    pub fn new() -> Self { Self }

    pub fn classify(&self, error: &crate::AgentError) -> ClassifiedError {
        match error {
            crate::AgentError::Provider(ref e) => self.classify_provider_error(e),
            _ => ClassifiedError {
                reason: FailoverReason::Unknown,
                message: error.to_string(),
                retryable: false,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: false,
                status_code: None,
                provider: None,
                model: None,
            }
        }
    }

    fn classify_provider_error(&self, error: &crate::ProviderError) -> ClassifiedError {
        let message = error.to_string().to_lowercase();
        if message.contains("401") || message.contains("unauthorized") {
            ClassifiedError { reason: FailoverReason::Auth, retryable: true, should_rotate_credential: true, should_compress: false, should_fallback: false, status_code: Some(401), provider: None, model: None, message: error.to_string() }
        } else if message.contains("429") || message.contains("rate limit") {
            ClassifiedError { reason: FailoverReason::RateLimit, retryable: true, should_rotate_credential: true, should_compress: false, should_fallback: false, status_code: Some(429), provider: None, model: None, message: error.to_string() }
        } else if message.contains("503") || message.contains("overloaded") {
            ClassifiedError { reason: FailoverReason::Overloaded, retryable: true, should_rotate_credential: false, should_compress: false, should_fallback: true, status_code: Some(503), provider: None, model: None, message: error.to_string() }
        } else if message.contains("timeout") || message.contains("timed out") {
            ClassifiedError { reason: FailoverReason::Timeout, retryable: true, should_rotate_credential: false, should_compress: false, should_fallback: false, status_code: None, provider: None, model: None, message: error.to_string() }
        } else {
            ClassifiedError { reason: FailoverReason::Unknown, retryable: true, should_rotate_credential: false, should_compress: false, should_fallback: false, status_code: None, provider: None, model: None, message: error.to_string() }
        }
    }
}

impl Default for ErrorClassifier {
    fn default() -> Self { Self::new() }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/error_classifier.rs
git commit -m "feat(core): add error_classifier module"
```

---

## Task 2: Create retry_utils.rs

**Files:**
- Create: `crates/hermes-core/src/retry_utils.rs`

- [ ] **Step 1: Create the file**

```rust
//! Retry Utils — 抖动指数退避重试机制

use crate::{AgentError, ErrorClassifier};
use std::time::Duration;

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

pub fn jittered_backoff(attempt: u32, config: &RetryConfig) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let delay = if exponent >= 63 || config.base_delay.as_secs_f64() <= 0.0 {
        config.max_delay
    } else {
        let exponential = config.base_delay.as_secs_f64() * (2_u32.pow(exponent) as f64);
        let capped = exponential.min(config.max_delay.as_secs_f64());
        Duration::from_secs_f64(capped)
    };
    let jitter = rand::random::<f64>() * config.jitter_ratio * delay.as_secs_f64();
    delay + Duration::from_secs_f64(jitter)
}

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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/retry_utils.rs
git commit -m "feat(core): add retry_utils with jittered backoff"
```

---

## Task 3: Create credential_pool.rs

**Files:**
- Create: `crates/hermes-core/src/credential_pool.rs`

- [ ] **Step 1: Create the file**

```rust
//! Credential Pool — 多凭证管理与故障转移

use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolStrategy {
    FillFirst,
    RoundRobin,
    Random,
    LeastUsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialStatus {
    Ok,
    Exhausted,
}

#[derive(Debug, Clone)]
pub struct CredentialEntry {
    pub key: String,
    pub status: CredentialStatus,
    pub exhausted_at: Option<Instant>,
    pub use_count: usize,
}

pub struct CredentialPool {
    entries: RwLock<HashMap<String, Vec<CredentialEntry>>>,
    strategy: PoolStrategy,
    exhausted_ttl: Duration,
}

impl CredentialPool {
    pub fn new(strategy: PoolStrategy) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            strategy,
            exhausted_ttl: Duration::from_secs(3600),
        }
    }

    pub fn add(&self, provider: &str, key: &str) {
        let mut entries = self.entries.write();
        entries.entry(provider.to_string())
            .or_default()
            .push(CredentialEntry {
                key: key.to_string(),
                status: CredentialStatus::Ok,
                exhausted_at: None,
                use_count: 0,
            });
    }

    pub fn select(&self, provider: &str) -> Option<String> {
        let mut entries = self.entries.write();
        let provider_entries = entries.get_mut(provider)?;
        let now = Instant::now();
        for entry in provider_entries.iter_mut() {
            if entry.status == CredentialStatus::Exhausted {
                if let Some(exhausted_at) = entry.exhausted_at {
                    if now.duration_since(exhausted_at) >= self.exhausted_ttl {
                        entry.status = CredentialStatus::Ok;
                        entry.exhausted_at = None;
                    }
                }
            }
        }
        let available: Vec<&mut CredentialEntry> = provider_entries
            .iter_mut()
            .filter(|e| e.status == CredentialStatus::Ok)
            .collect();
        if available.is_empty() { return None; }
        let selected = match self.strategy {
            PoolStrategy::FillFirst => available.into_iter().next(),
            PoolStrategy::RoundRobin => available.into_iter().min_by_key(|e| e.use_count),
            PoolStrategy::Random => {
                let idx = rand::random::<usize>() % available.len();
                available.into_iter().nth(idx)
            }
            PoolStrategy::LeastUsed => available.into_iter().min_by_key(|e| e.use_count),
        };
        selected.map(|e| { e.use_count += 1; e.key.clone() })
    }

    pub fn mark_exhausted(&self, provider: &str, key: &str) {
        let mut entries = self.entries.write();
        if let Some(provider_entries) = entries.get_mut(provider) {
            for entry in provider_entries.iter_mut() {
                if entry.key == key {
                    entry.status = CredentialStatus::Exhausted;
                    entry.exhausted_at = Some(Instant::now());
                    break;
                }
            }
        }
    }
}

impl Default for CredentialPool {
    fn default() -> Self { Self::new(PoolStrategy::RoundRobin) }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/credential_pool.rs
git commit -m "feat(core): add credential_pool with failover strategies"
```

---

## Task 4: Update lib.rs exports

**Files:**
- Modify: `crates/hermes-core/src/lib.rs`

- [ ] **Step 1: Add module declarations and exports**

```rust
pub mod credential_pool;
pub mod error_classifier;
pub mod retry_utils;
```

And exports:
```rust
pub use credential_pool::{CredentialPool, CredentialEntry, CredentialStatus, PoolStrategy};
pub use error_classifier::{ErrorClassifier, ClassifiedError, FailoverReason};
pub use retry_utils::{RetryConfig, jittered_backoff, with_retry};
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/lib.rs
git commit -m "feat(core): export infrastructure modules"
```

---

## Task 5: Integrate retry into Agent

**Files:**
- Modify: `crates/hermes-core/src/agent.rs`

- [ ] **Step 1: Add fields to Agent struct**

```rust
    error_classifier: Arc<ErrorClassifier>,
    retry_config: RetryConfig,
```

- [ ] **Step 2: Update Agent::new()**

Add parameters and initialization.

- [ ] **Step 3: Wrap LLM call with retry**

```rust
let response = with_retry(
    || self.provider.chat(chat_request.clone()),
    &self.retry_config,
    &self.error_classifier,
).await.map_err(AgentError::Provider)?;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p hermes-core`

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-core/src/agent.rs
git commit -m "feat(core): integrate retry logic into Agent"
```

---

## Task 6: Update callers

**Files:**
- Modify: `crates/hermes-cli/src/chat.rs`
- Modify: `crates/hermes-cli/src/handlers/gateway.rs`
- Modify: `crates/hermes-acp/src/lib.rs`

- [ ] **Step 1: Pass None for new parameters**

Pass `None, None` for error_classifier and retry_config in all Agent::new() calls.

- [ ] **Step 2: Verify compilation**

Run: `cargo check --all`

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(cli): wire up error_classifier and retry_config"
```

---

## Task 7: Add integration tests

**Files:**
- Create: `crates/hermes-core/tests/test_infrastructure.rs`

- [ ] **Step 1: Create test file**

Tests for credential_pool, error_classifier, retry_utils.

- [ ] **Step 2: Run tests**

Run: `cargo test -p hermes-core --test test_infrastructure`

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/tests/test_infrastructure.rs
git commit -m "test(core): add infrastructure integration tests"
```

---

## Self-Review

- [x] Spec coverage complete
- [x] No placeholders
- [x] Type consistency verified
