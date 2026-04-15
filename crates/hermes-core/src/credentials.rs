//! 凭证池管理模块
//!
//! ## 模块用途
//! 管理多个 API 凭证（API Key）的健康状态、负载均衡和自动冷却。
//! 支持多凭证轮询（round-robin），并在失败时自动将凭证置入冷却期。
//!
//! ## 主要类型
//! - **CredentialHealth**: 单个凭证的健康状态（可用性、冷却截止时间、连续失败次数）
//! - **CredentialPool**: 凭证池，管理多个凭证的生命周期
//!
//! ## 工作原理
//! - **添加凭证**: `add(name, key)` — 将新凭证加入池中
//! - **获取凭证**: `get()` — 使用轮询策略从健康的凭证中返回一个（name, key）
//! - **报告失败**: `report_failure()` — 累计 3 次失败后，凭证进入 60 秒冷却期
//! - **报告限流**: `report_rate_limit()` — 凭证立即进入冷却（至少 60 秒）
//! - **报告成功**: `report_success()` — 清除失败计数，恢复凭证可用性
//! - **健康检查**: `health()` — 获取所有凭证的当前健康状态
//!
//! ## 与其他模块的关系
//! - 被 `retrying_provider.rs` 使用来管理多凭证的负载均衡
//! - 凭证从 `Config` 的 `credentials` 字段加载
//! - 使用 `parking_lot::RwLock` 实现内部并发安全（读写锁）

use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Health state of a single credential.
#[derive(Debug, Clone)]
pub struct CredentialHealth {
    pub name: String,
    pub available: bool,
    pub cooldown_until: Option<Instant>,
    pub failures: u32,
}

impl CredentialHealth {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            available: true,
            cooldown_until: None,
            failures: 0,
        }
    }
}

/// A pool of named credentials with health tracking and automatic cooldown.
pub struct CredentialPool {
    inner: RwLock<Inner>,
    cooldown: Duration,
}

struct Inner {
    credentials: HashMap<String, String>,
    health: HashMap<String, CredentialHealth>,
    order: Vec<String>,
    rr_cursor: usize,
}

impl CredentialPool {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                credentials: HashMap::new(),
                health: HashMap::new(),
                order: Vec::new(),
                rr_cursor: 0,
            }),
            cooldown: Duration::from_secs(60),
        }
    }

    /// Add a credential to the pool.
    pub fn add(&self, name: impl Into<String>, key: String) {
        let name = name.into();
        let mut inner = self.inner.write();
        inner.credentials.insert(name.clone(), key);
        inner.health.insert(name.clone(), CredentialHealth::new(name.clone()));
        if !inner.order.contains(&name) {
            inner.order.push(name);
        }
    }

    /// Remove a credential from the pool.
    pub fn remove(&self, name: &str) {
        let mut inner = self.inner.write();
        inner.credentials.remove(name);
        inner.health.remove(name);
        inner.order.retain(|n| n != name);
    }

    /// Get a healthy credential using round-robin. Returns (name, key) or None.
    pub fn get(&self) -> Option<(String, String)> {
        // Must use write() because we mutate rr_cursor
        let mut inner = self.inner.write();
        let now = Instant::now();
        let start_len = inner.order.len();

        for _ in 0..start_len {
            let idx = inner.rr_cursor % start_len;
            inner.rr_cursor += 1;
            let name: String = inner.order[idx].clone();

            // Check cooldown expiry inline
            let available = {
                let health = inner.health.get(&name)?;
                if health.available {
                    true
                } else if let Some(until) = health.cooldown_until {
                    now >= until
                } else {
                    false
                }
            };

            if available {
                if let Some(key) = inner.credentials.get(&name) {
                    return Some((name, key.clone()));
                }
            }
        }
        None
    }

    /// Get all credential names.
    pub fn names(&self) -> Vec<String> {
        self.inner.read().order.clone()
    }

    /// Report a rate-limit event for a credential.
    pub fn report_rate_limit(&self, name: &str, retry_after_secs: u64) {
        let mut inner = self.inner.write();
        if let Some(health) = inner.health.get_mut(name) {
            health.available = false;
            health.failures += 1;
            health.cooldown_until =
                Some(Instant::now() + Duration::from_secs(retry_after_secs.max(60)));
        }
    }

    /// Report a failure for a credential.
    pub fn report_failure(&self, name: &str) {
        let mut inner = self.inner.write();
        if let Some(health) = inner.health.get_mut(name) {
            health.failures += 1;
            if health.failures >= 3 {
                health.available = false;
                health.cooldown_until = Some(Instant::now() + self.cooldown);
            }
        }
    }

    /// Report a success, clearing failure count.
    pub fn report_success(&self, name: &str) {
        let mut inner = self.inner.write();
        if let Some(health) = inner.health.get_mut(name) {
            health.failures = 0;
            if !health.available {
                health.available = true;
                health.cooldown_until = None;
            }
        }
    }

    /// Get health status for all credentials.
    pub fn health(&self) -> Vec<CredentialHealth> {
        self.inner.read().health.values().cloned().collect()
    }
}

impl Default for CredentialPool {
    fn default() -> Self {
        Self::new()
    }
}
