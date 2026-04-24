# Analysis & Insights Features Design Spec

**Goal:** 实现用量追踪与数据分析功能，支持 CLI 实时显示和结构化日志输出

**Architecture:** 三个模块放入 `hermes-core`：`insights.rs`、`usage_pricing.rs`、`rate_limit_tracker.rs`

**Tech Stack:** Rust, serde_json, tokio

---

## Overview

本模块提供三类洞察功能：

1. **会话洞察引擎 (insights)** — 追踪 LLM token 消耗、工具调用逐次记录
2. **成本估算与定价数据库 (usage_pricing)** — 硬编码定价表，计算 USD 费用
3. **速率限制追踪器 (rate_limit_tracker)** — 从 ProviderError 捕获限流事件并输出 JSON 日志

---

## Module 1: insights.rs — 会话洞察引擎

### Types

```rust
/// 单次工具调用记录
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub started_at: f64,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
}

/// 会话洞察数据
#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionInsights {
    pub session_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_read_tokens: usize,
    pub cache_write_tokens: usize,
    pub reasoning_tokens: usize,
    pub estimated_cost_usd: f64,
    pub tool_calls: Vec<ToolCallRecord>,
}

/// Insights trait — 可自定义记录行为
pub trait InsightsTracker: Send + Sync {
    fn record_tool_call(&self, record: ToolCallRecord);
    fn record_usage(&self, usage: &Usage);
    fn get_insights(&self) -> SessionInsights;
}

/// 默认实现：内存存储
pub struct InMemoryInsightsTracker {
    insights: Mutex<SessionInsights>,
}
```

### Integration Points

- Agent 在工具调用前后分别调用 `insights.record_tool_call()`
- Agent 在 LLM 响应后调用 `insights.record_usage()` 并计算成本
- Session 结束时汇总写入日志

---

## Module 2: usage_pricing.rs — 定价数据库

### Types

```rust
/// 定价层
#[derive(Debug, Clone)]
pub struct PricingTier {
    pub input_per_million: f64,       // $ / 1M input tokens
    pub output_per_million: f64,       // $ / 1M output tokens
    pub cache_read_per_million: f64,   // $ / 1M cache read tokens
    pub cache_write_per_million: f64,  // $ / 1M cache write tokens
}

/// 定价数据库
pub struct PricingDatabase {
    tiers: HashMap<String, HashMap<String, PricingTier>>, // provider -> model -> pricing
}
```

### Hardcoded Pricing

```rust
impl PricingDatabase {
    pub fn new() -> Self {
        let mut db = Self { tiers: HashMap::new() };
        
        // OpenAI
        db.tiers.insert("openai".into(), HashMap::from([
            ("gpt-4o".into(), PricingTier {
                input_per_million: 5.00,
                output_per_million: 15.00,
                cache_read_per_million: 1.25,
                cache_write_per_million: 10.00,
            }),
            ("gpt-4o-mini".into(), PricingTier {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cache_read_per_million: 0.04,
                cache_write_per_million: 0.50,
            }),
        ]));
        
        // Anthropic
        db.tiers.insert("anthropic".into(), HashMap::from([
            ("claude-3-5-sonnet-20241022".into(), PricingTier {
                input_per_million: 3.00,
                output_per_million: 15.00,
                cache_read_per_million: 0.30,
                cache_write_per_million: 3.75,
            }),
        ]));
        
        db
    }
    
    pub fn get_pricing(&self, provider: &str, model: &str) -> Option<&PricingTier> {
        self.tiers.get(provider)?.get(model)
    }
}

/// 成本计算器
pub struct CostCalculator<'a> {
    pricing: &'a PricingDatabase,
}

impl CostCalculator<'_> {
    pub fn calculate(&self, provider: &str, model: &str, usage: &Usage) -> Option<f64> {
        let tier = self.pricing.get_pricing(provider, model)?;
        
        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * tier.input_per_million;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * tier.output_per_million;
        let cache_read_cost = usage.cache_read_tokens
            .map(|t| (t as f64 / 1_000_000.0) * tier.cache_read_per_million)
            .unwrap_or(0.0);
        let cache_write_cost = usage.cache_write_tokens
            .map(|t| (t as f64 / 1_000_000.0) * tier.cache_write_per_million)
            .unwrap_or(0.0);
        
        Some(input_cost + output_cost + cache_read_cost + cache_write_cost)
    }
}
```

---

## Module 3: rate_limit_tracker.rs — 速率限制追踪器

### Types

```rust
/// Rate Limit 事件
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitEvent {
    pub event: String,           // "rate_limited"
    pub provider: String,
    pub retry_after_secs: u64,
    pub timestamp: f64,
}

/// Tracker 实现
pub struct RateLimitTracker;

impl RateLimitTracker {
    pub fn new() -> Self {
        Self
    }
    
    /// 记录限流事件并输出 JSON 日志
    pub fn record(&self, provider: &str, retry_after: u64) {
        let event = RateLimitEvent {
            event: "rate_limited".into(),
            provider: provider.into(),
            retry_after_secs: retry_after,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        };
        
        // 输出到 stdout（结构化日志）
        println!("{}", serde_json::to_string(&event).unwrap());
    }
}
```

---

## DisplayHandler Extension

### Trait Changes

```rust
/// 显示处理 trait — Agent 工具执行和思考的显示反馈
pub trait DisplayHandler: Send + Sync {
    // ... existing methods ...

    /// 显示 LLM 调用用量（新增）
    fn show_usage(&self, insights: &SessionInsights);
}
```

### Default Implementation

```rust
impl DisplayHandler for NoopDisplay {
    // ... existing implementations ...
    
    fn show_usage(&self, _insights: &SessionInsights) {}
}
```

---

## Log Output Format

### Rate Limit Event

```json
{"event": "rate_limited", "provider": "openai", "retry_after": 60, "timestamp": 1745500000.123}
```

### LLM Usage Event

```json
{"event": "llm_usage", "session": "abc123", "provider": "openai", "model": "gpt-4o", "input_tokens": 1200, "output_tokens": 3400, "cache_read_tokens": 500, "cost_usd": 0.042, "timestamp": 1745500000.456}
```

---

## CLI Display Format

```
[Tokens: 1.2K/3.4K | Cost: $0.042 | Tools: ReadFile(2), WriteFile(1)]
```

其中：
- `1.2K/3.4K` = input tokens / output tokens（简写）
- `$0.042` = 本次 LLM 调用的估算费用
- `ReadFile(2), WriteFile(1)` = 工具调用统计

---

## File Structure

```
crates/hermes-core/src/
├── insights.rs           # 会话洞察引擎 (~150 lines)
├── usage_pricing.rs     # 定价数据库 (~200 lines)
├── rate_limit_tracker.rs # Rate Limit 追踪器 (~80 lines)
├── display.rs           # 修改：添加 show_usage 方法
├── agent.rs             # 修改：集成 insights tracker
└── lib.rs               # 修改：导出新模块
```

---

## Integration with Agent

Agent 在构造时接收 `InsightsTracker` 和 `RateLimitTracker`：

```rust
pub struct Agent {
    // ... existing fields ...
    
    insights_tracker: Option<Arc<dyn InsightsTracker>>,
    rate_limit_tracker: Option<Arc<RateLimitTracker>>,
}
```

工具调用时：

```rust
// tool_dispatch.rs
if let Some(tracker) = &self.insights_tracker {
    tracker.record_tool_call(ToolCallRecord {
        tool_name: call.name.clone(),
        started_at: now,
        duration_ms: elapsed,
        success: true,
        error: None,
    });
}
```

---

## Self-Review Checklist

- [x] 没有 "TBD" 或未完成的部分
- [x] 类型签名清晰、一致
- [x] 模块边界清晰：insights 只追踪、pricing 只定价、tracker 只记录
- [x] DisplayHandler 扩展向后兼容（默认 NoopDisplay 实现为空）
- [x] JSON 日志格式简单、可解析
