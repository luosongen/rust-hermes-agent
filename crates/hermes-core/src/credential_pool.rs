//! Credential Pool — 多凭证管理与故障转移
//!
//! ## 整合说明 (2026-04-26)
//! 本模块由两个旧模块整合而成:
//! - `credentials.rs`: 简单的 round-robin 实现,带健康检查和冷却
//! - `credential_pool.rs`: 支持多种策略(FillFirst/RoundRobin/Random/LeastUsed)
//!
//! 整合后保留 `credential_pool.rs` 的策略框架,同时添加 `credentials.rs` 的:
//! - 基于凭证名的 API (`get()`, `report_failure()`, `report_rate_limit()`, `report_success()`)
//! - `CredentialHealth` 健康状态追踪
//! - 自动冷却机制 (3次失败或限流后)
//!
//! ## 支持的策略
//! - **FillFirst**: 先用满第一个凭证
//! - **RoundRobin**: 轮询选择
//! - **Random**: 随机选择
//! - **LeastUsed**: 选择使用次数最少

use rand::Rng;
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
    pub name: String,        // 凭证名称(标识符)
    pub key: String,         // API Key
    pub status: CredentialStatus,
    pub exhausted_at: Option<Instant>,
    pub use_count: usize,
    pub failures: u32,       // 连续失败次数
    pub cooldown_until: Option<Instant>,  // 冷却截止时间
}

impl CredentialEntry {
    fn new(name: String, key: String) -> Self {
        Self {
            name,
            key,
            status: CredentialStatus::Ok,
            exhausted_at: None,
            use_count: 0,
            failures: 0,
            cooldown_until: None,
        }
    }
}

/// 凭证健康状态 (用于 health() 方法返回)
#[derive(Debug, Clone)]
pub struct CredentialHealth {
    pub name: String,
    pub available: bool,
    pub cooldown_until: Option<Instant>,
    pub failures: u32,
}

/// 凭证池
pub struct CredentialPool {
    entries: RwLock<HashMap<String, Vec<CredentialEntry>>>,  // provider -> entries
    strategy: PoolStrategy,
    // 耗尽凭证的默认冷却时间
    exhausted_ttl: Duration,
    // round-robin 游标 (用于 RoundRobin 策略)
    rr_cursor: RwLock<usize>,
    // 默认冷却时间 (用于 report_failure)
    default_cooldown: Duration,
}

impl CredentialPool {
    pub fn new(strategy: PoolStrategy) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            strategy,
            exhausted_ttl: Duration::from_secs(3600), // 1小时
            rr_cursor: RwLock::new(0),
            default_cooldown: Duration::from_secs(60),
        }
    }

    /// 添加凭证到指定 provider (name 用于标识凭证)
    pub fn add(&self, provider: &str, name: &str, key: &str) {
        let mut entries = self.entries.write();
        entries.entry(provider.to_string())
            .or_default()
            .push(CredentialEntry::new(name.to_string(), key.to_string()));
    }

    /// 获取凭证名列表
    pub fn names(&self) -> Vec<String> {
        let entries = self.entries.read();
        entries.values()
            .flat_map(|v| v.iter().map(|e| e.name.clone()))
            .collect()
    }

    /// 获取健康状态
    pub fn health(&self) -> Vec<CredentialHealth> {
        let entries = self.entries.read();
        entries.values()
            .flat_map(|v| v.iter().map(|e| {
                let available = e.status == CredentialStatus::Ok &&
                    e.cooldown_until.map_or(true, |until| Instant::now() >= until);
                CredentialHealth {
                    name: e.name.clone(),
                    available,
                    cooldown_until: e.cooldown_until,
                    failures: e.failures,
                }
            }))
            .collect()
    }

    /// 标记凭证为耗尽 (限流时调用)
    pub fn mark_exhausted(&self, provider: &str, name: &str) {
        let mut entries = self.entries.write();
        if let Some(provider_entries) = entries.get_mut(provider) {
            for entry in provider_entries.iter_mut() {
                if entry.name == name {
                    entry.status = CredentialStatus::Exhausted;
                    entry.exhausted_at = Some(Instant::now());
                    break;
                }
            }
        }
    }

    /// 报告失败 (累计3次后进入冷却)
    pub fn report_failure(&self, provider: &str, name: &str) {
        let mut entries = self.entries.write();
        if let Some(provider_entries) = entries.get_mut(provider) {
            for entry in provider_entries.iter_mut() {
                if entry.name == name {
                    entry.failures += 1;
                    if entry.failures >= 3 {
                        entry.cooldown_until = Some(Instant::now() + Duration::from_secs(60));
                    }
                    break;
                }
            }
        }
    }

    /// 报告限流 (立即进入冷却)
    pub fn report_rate_limit(&self, provider: &str, name: &str, retry_after_secs: u64) {
        let mut entries = self.entries.write();
        if let Some(provider_entries) = entries.get_mut(provider) {
            for entry in provider_entries.iter_mut() {
                if entry.name == name {
                    entry.cooldown_until = Some(Instant::now() + Duration::from_secs(retry_after_secs.max(60)));
                    break;
                }
            }
        }
    }

    /// 报告成功 (清除失败计数)
    pub fn report_success(&self, provider: &str, name: &str) {
        let mut entries = self.entries.write();
        if let Some(provider_entries) = entries.get_mut(provider) {
            for entry in provider_entries.iter_mut() {
                if entry.name == name {
                    entry.failures = 0;
                    entry.cooldown_until = None;
                    break;
                }
            }
        }
    }

    /// 选择下一个可用凭证,返回 (name, key) 或 None
    pub fn get(&self, provider: &str) -> Option<(String, String)> {
        let mut entries = self.entries.write();
        let provider_entries = entries.get_mut(provider)?;

        if provider_entries.is_empty() {
            return None;
        }

        // 清理过期的 exhausted 凭证,重置冷却中的凭证
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
            // 清理冷却过期
            if let Some(until) = entry.cooldown_until {
                if now >= until {
                    entry.cooldown_until = None;
                }
            }
        }

        // 收集可用凭证
        let available: Vec<&mut CredentialEntry> = provider_entries
            .iter_mut()
            .filter(|e| {
                e.status == CredentialStatus::Ok &&
                e.cooldown_until.is_none()
            })
            .collect();

        if available.is_empty() {
            return None;
        }

        // 根据策略选择
        let selected = match self.strategy {
            PoolStrategy::FillFirst => available.into_iter().next(),
            PoolStrategy::RoundRobin => {
                let mut cursor = self.rr_cursor.write();
                let start_len = available.len();
                let idx = *cursor % start_len;
                *cursor = (*cursor + 1) % start_len;
                available.into_iter().nth(idx)
            }
            PoolStrategy::Random => {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..available.len());
                available.into_iter().nth(idx)
            }
            PoolStrategy::LeastUsed => {
                available.into_iter().min_by_key(|e| e.use_count)
            }
        };

        selected.map(|e| {
            e.use_count += 1;
            (e.name.clone(), e.key.clone())
        })
    }
}

impl Default for CredentialPool {
    fn default() -> Self {
        Self::new(PoolStrategy::RoundRobin)
    }
}

/// 敏感值封装类型,序列化时显示为 `<redacted>`
#[derive(Debug, Clone)]
pub struct Secret<T>(pub T);

impl<T> serde::Serialize for Secret<T>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("<redacted>")
    }
}

impl<'de, T> serde::Deserialize<'de> for Secret<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Secret(T::deserialize(deserializer)?))
    }
}
