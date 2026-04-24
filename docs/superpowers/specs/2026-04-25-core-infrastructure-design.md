# 核心基础设施设计规范

**目标：** 实现凭证池、错误分类器和重试机制，提升 Agent 的稳定性和容错能力

**架构：** 混合方案 — 凭证池作为独立模块，错误分类和重试并入 Agent 核心流程

**技术栈：** Rust, tokio, parking_lot, serde

---

## 概述

本模块提供三个核心基础设施组件：

1. **凭证池 (credential_pool)** — 多凭证管理，支持故障转移和负载均衡
2. **错误分类器 (error_classifier)** — 结构化错误分类，决定恢复策略
3. **重试工具 (retry_utils)** — 抖动指数退避，防止惊群效应

---

## 模块 1: credential_pool.rs — 凭证池

### 类型

```rust
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
```

### 实现

```rust
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
```

---

## 模块 2: error_classifier.rs — 错误分类器

### 类型

```rust
/// 故障原因分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverReason {
    Auth,                    // 认证失败（401/403）— 刷新/轮换
    AuthPermanent,          // 认证永久失败 — 中止
    Billing,                // 计费/配额耗尽（402）— 立即轮换
    RateLimit,              // 速率限制（429）— 退避后轮换
    Overloaded,             // 服务过载（503/529）— 退避
    ServerError,            // 服务器错误（500/502）— 重试
    Timeout,                // 超时 — 重建客户端 + 重试
    ContextOverflow,        // 上下文过大 — 压缩
    PayloadTooLarge,        // 负载过大（413）— 压缩
    ModelNotFound,          // 模型不存在（404）— 降级
    FormatError,            // 格式错误（400）— 中止或清理后重试
    Unknown,                // 未知 — 退避后重试
}

/// 分类结果
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

/// 错误分类器
pub struct ErrorClassifier;
```

### 实现

```rust
impl ErrorClassifier {
    pub fn new() -> Self {
        Self
    }

    pub fn classify(&self, error: &AgentError) -> ClassifiedError {
        match error {
            AgentError::Provider(ref e) => {
                self.classify_provider_error(e)
            }
            AgentError::Tool(ref e) => {
                ClassifiedError {
                    reason: FailoverReason::Unknown,
                    message: e.to_string(),
                    retryable: false,
                    should_compress: false,
                    should_rotate_credential: false,
                    should_fallback: false,
                    status_code: None,
                    provider: None,
                    model: None,
                }
            }
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

    fn classify_provider_error(&self, error: &ProviderError) -> ClassifiedError {
        // 根据错误消息和状态码分类
        let message = error.to_string().to_lowercase();

        if message.contains("401") || message.contains("unauthorized") {
            ClassifiedError {
                reason: FailoverReason::Auth,
                retryable: true,
                should_rotate_credential: true,
                should_compress: false,
                should_fallback: false,
                status_code: Some(401),
                provider: None,
                model: None,
                message: error.to_string(),
            }
        } else if message.contains("429") || message.contains("rate limit") {
            ClassifiedError {
                reason: FailoverReason::RateLimit,
                retryable: true,
                should_rotate_credential: true,
                should_compress: false,
                should_fallback: false,
                status_code: Some(429),
                provider: None,
                model: None,
                message: error.to_string(),
            }
        } else if message.contains("503") || message.contains("overloaded") {
            ClassifiedError {
                reason: FailoverReason::Overloaded,
                retryable: true,
                should_rotate_credential: false,
                should_compress: false,
                should_fallback: true,
                status_code: Some(503),
                provider: None,
                model: None,
                message: error.to_string(),
            }
        } else if message.contains("timeout") || message.contains("timed out") {
            ClassifiedError {
                reason: FailoverReason::Timeout,
                retryable: true,
                should_rotate_credential: false,
                should_compress: false,
                should_fallback: false,
                status_code: None,
                provider: None,
                model: None,
                message: error.to_string(),
            }
        } else {
            ClassifiedError {
                reason: FailoverReason::Unknown,
                retryable: true,
                should_rotate_credential: false,
                should_compress: false,
                should_fallback: false,
                status_code: None,
                provider: None,
                model: None,
                message: error.to_string(),
            }
        }
    }
}
```

---

## 模块 3: retry_utils.rs — 重试工具

### 类型

```rust
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
            max_attempts: 3,
            base_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(120),
            jitter_ratio: 0.5,
        }
    }
}
```

### 实现

```rust
/// 计算抖动指数退避延迟
pub fn jittered_backoff(
    attempt: u32,
    config: &RetryConfig,
) -> Duration {
    let exponent = attempt.saturating_sub(1) as u32;
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
```

---

## 与 Agent 的集成

### Agent 结构更新

```rust
pub struct Agent {
    // ... 现有字段 ...
    credential_pool: Option<Arc<CredentialPool>>,
    error_classifier: Arc<ErrorClassifier>,
    retry_config: RetryConfig,
}
```

### LLM 调用流程更新

```rust
// 在 run_conversation 中
let chat_request = ChatRequest { /* ... */ };

let response = with_retry(
    || self.provider.chat(chat_request.clone()),
    &self.retry_config,
    &self.error_classifier,
).await?;
```

---

## 文件结构

```
crates/hermes-core/src/
├── credential_pool.rs      # 凭证池 (~200 lines)
├── error_classifier.rs     # 错误分类器 (~150 lines)
├── retry_utils.rs          # 重试工具 (~100 lines)
├── agent.rs               # 修改：集成三个模块
└── lib.rs                 # 修改：导出新模块

crates/hermes-core/tests/
└── test_infrastructure.rs  # 集成测试
```

---

## 自检清单

- [x] 没有 "TBD" 或未完成的部分
- [x] 类型签名清晰、一致
- [x] 模块边界清晰
- [x] 错误分类覆盖主要场景
- [x] 重试机制防止惊群效应
- [x] 凭证池支持多种策略
