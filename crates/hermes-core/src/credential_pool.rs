//! Credential Pool — 多凭证管理与故障转移
//!
//! 支持多种选择策略，凭证耗尽后自动冷却恢复。

use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 凭证池策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolStrategy {
    FillFirst,    // 先用满第一个凭证
    RoundRobin,   // 轮询
    Random,       // 随机选择
    LeastUsed,    // 最少使用
}

/// 凭证状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialStatus {
    Ok,
    Exhausted,    // 已耗尽（限流/配额用完）
}

/// 单个凭证条目
#[derive(Debug, Clone)]
pub struct CredentialEntry {
    pub key: String,
    pub status: CredentialStatus,
    pub exhausted_at: Option<Instant>,
    pub use_count: usize,
}

/// 凭证池
pub struct CredentialPool {
    entries: RwLock<HashMap<String, Vec<CredentialEntry>>>,
    strategy: PoolStrategy,
    // 耗尽凭证的默认冷却时间
    exhausted_ttl: Duration,
}

impl CredentialPool {
    pub fn new(strategy: PoolStrategy) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            strategy,
            exhausted_ttl: Duration::from_secs(3600), // 1小时
        }
    }

    /// 添加凭证到指定 provider
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

    /// 选择下一个可用凭证
    pub fn select(&self, provider: &str) -> Option<String> {
        let mut entries = self.entries.write();
        let provider_entries = entries.get_mut(provider)?;

        // 清理已过期的 exhausted 凭证
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

        // 根据策略选择
        let available: Vec<&mut CredentialEntry> = provider_entries
            .iter_mut()
            .filter(|e| e.status == CredentialStatus::Ok)
            .collect();

        if available.is_empty() {
            return None;
        }

        let selected = match self.strategy {
            PoolStrategy::FillFirst => available.into_iter().next(),
            PoolStrategy::RoundRobin => {
                // 找到 use_count 最小的
                available.into_iter().min_by_key(|e| e.use_count)
            }
            PoolStrategy::Random => {
                let idx = rand::random::<usize>() % available.len();
                available.into_iter().nth(idx)
            }
            PoolStrategy::LeastUsed => {
                available.into_iter().min_by_key(|e| e.use_count)
            }
        };

        selected.map(|e| {
            e.use_count += 1;
            e.key.clone()
        })
    }

    /// 标记凭证为耗尽
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
    fn default() -> Self {
        Self::new(PoolStrategy::RoundRobin)
    }
}
