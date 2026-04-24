# Insights & Analytics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现用量追踪与数据分析功能，包括会话洞察引擎、定价数据库、速率限制追踪器

**Architecture:** 三个新模块放入 hermes-core：insights.rs、usage_pricing.rs、rate_limit_tracker.rs；扩展 DisplayHandler trait 添加 show_usage 方法；集成到 Agent

**Tech Stack:** Rust, serde_json, tokio, parking_lot

---

## File Structure

```
crates/hermes-core/src/
├── insights.rs              # 新增 (~150 lines)
├── usage_pricing.rs         # 新增 (~200 lines)
├── rate_limit_tracker.rs    # 新增 (~80 lines)
├── display.rs               # 修改：添加 show_usage 方法
├── agent.rs                 # 修改：集成 trackers
└── lib.rs                  # 修改：导出新模块

crates/hermes-core/tests/
└── test_insights.rs        # 新增
```

---

## Task 1: Create insights.rs — 会话洞察引擎

**Files:**
- Create: `crates/hermes-core/src/insights.rs`

- [ ] **Step 1: Create the file with types and trait**

```rust
//! Insights — 会话洞察引擎
//!
//! 追踪 LLM token 消耗和工具调用记录。

use crate::{ModelId, Usage};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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

impl SessionInsights {
    pub fn new(session_id: &str, provider: &str, model: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            ..Default::default()
        }
    }
}

/// Insights tracker trait
pub trait InsightsTracker: Send + Sync {
    fn record_tool_call(&self, record: ToolCallRecord);
    fn record_usage(&self, usage: &Usage, cost_usd: f64);
    fn get_insights(&self) -> SessionInsights;
}

/// 内存存储的 tracker 实现
pub struct InMemoryInsightsTracker {
    insights: Mutex<SessionInsights>,
}

impl InMemoryInsightsTracker {
    pub fn new(session_id: &str, provider: &str, model: &str) -> Self {
        Self {
            insights: Mutex::new(SessionInsights::new(session_id, provider, model)),
        }
    }
}

impl InsightsTracker for InMemoryInsightsTracker {
    fn record_tool_call(&self, record: ToolCallRecord) {
        let mut insights = self.insights.lock();
        insights.tool_calls.push(record);
    }

    fn record_usage(&self, usage: &Usage, cost_usd: f64) {
        let mut insights = self.insights.lock();
        insights.input_tokens = usage.input_tokens;
        insights.output_tokens = usage.output_tokens;
        insights.cache_read_tokens = usage.cache_read_tokens.unwrap_or(0);
        insights.cache_write_tokens = usage.cache_write_tokens.unwrap_or(0);
        insights.reasoning_tokens = usage.reasoning_tokens.unwrap_or(0);
        insights.estimated_cost_usd = cost_usd;
    }

    fn get_insights(&self) -> SessionInsights {
        self.insights.lock().clone()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/insights.rs
git commit -m "feat(core): add insights module with SessionInsights and InsightsTracker

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Create usage_pricing.rs — 定价数据库

**Files:**
- Create: `crates/hermes-core/src/usage_pricing.rs`

- [ ] **Step 1: Create the file with PricingDatabase**

```rust
//! Usage Pricing — 成本估算与定价数据库
//!
//! 提供各 Provider 的 token 定价和成本计算。

use crate::Usage;
use std::collections::HashMap;

/// 定价层
#[derive(Debug, Clone)]
pub struct PricingTier {
    /// $ / 1M input tokens
    pub input_per_million: f64,
    /// $ / 1M output tokens
    pub output_per_million: f64,
    /// $ / 1M cache read tokens
    pub cache_read_per_million: f64,
    /// $ / 1M cache write tokens
    pub cache_write_per_million: f64,
}

/// 定价数据库
pub struct PricingDatabase {
    tiers: HashMap<String, HashMap<String, PricingTier>>,
}

impl PricingDatabase {
    pub fn new() -> Self {
        let mut db = Self { tiers: HashMap::new() };

        // OpenAI
        db.tiers.insert("openai".to_string(), HashMap::from([
            ("gpt-4o".to_string(), PricingTier {
                input_per_million: 5.00,
                output_per_million: 15.00,
                cache_read_per_million: 1.25,
                cache_write_per_million: 10.00,
            }),
            ("gpt-4o-mini".to_string(), PricingTier {
                input_per_million: 0.15,
                output_per_million: 0.60,
                cache_read_per_million: 0.04,
                cache_write_per_million: 0.50,
            }),
            ("gpt-4-turbo".to_string(), PricingTier {
                input_per_million: 10.00,
                output_per_million: 30.00,
                cache_read_per_million: 1.25,
                cache_write_per_million: 10.00,
            }),
        ]));

        // Anthropic
        db.tiers.insert("anthropic".to_string(), HashMap::from([
            ("claude-3-5-sonnet-20241022".to_string(), PricingTier {
                input_per_million: 3.00,
                output_per_million: 15.00,
                cache_read_per_million: 0.30,
                cache_write_per_million: 3.75,
            }),
            ("claude-3-opus".to_string(), PricingTier {
                input_per_million: 15.00,
                output_per_million: 75.00,
                cache_read_per_million: 1.50,
                cache_write_per_million: 18.75,
            }),
            ("claude-3-haiku".to_string(), PricingTier {
                input_per_million: 0.25,
                output_per_million: 1.25,
                cache_read_per_million: 0.03,
                cache_write_per_million: 0.30,
            }),
        ]));

        // DeepSeek
        db.tiers.insert("deepseek".to_string(), HashMap::from([
            ("deepseek-chat".to_string(), PricingTier {
                input_per_million: 0.14,
                output_per_million: 0.28,
                cache_read_per_million: 0.01,
                cache_write_per_million: 0.14,
            }),
        ]));

        db
    }

    pub fn get_pricing(&self, provider: &str, model: &str) -> Option<&PricingTier> {
        self.tiers.get(provider)?.get(model)
    }
}

impl Default for PricingDatabase {
    fn default() -> Self {
        Self::new()
    }
}

/// 成本计算器
pub struct CostCalculator<'a> {
    pricing: &'a PricingDatabase,
}

impl<'a> CostCalculator<'a> {
    pub fn new(pricing: &'a PricingDatabase) -> Self {
        Self { pricing }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation_gpt4o() {
        let pricing = PricingDatabase::new();
        let calculator = CostCalculator::new(&pricing);

        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 2000,
            cache_read_tokens: Some(500),
            cache_write_tokens: Some(100),
            reasoning_tokens: None,
        };

        // 1000/1M * $5.00 = $0.005
        // 2000/1M * $15.00 = $0.030
        // 500/1M * $1.25 = $0.000625
        // 100/1M * $10.00 = $0.001
        // Total = $0.036625
        let cost = calculator.calculate("openai", "gpt-4o", &usage).unwrap();
        assert!((cost - 0.036625).abs() < 0.0001);
    }

    #[test]
    fn test_unknown_model_returns_none() {
        let pricing = PricingDatabase::new();
        let calculator = CostCalculator::new(&pricing);

        let usage = Usage {
            input_tokens: 100,
            output_tokens: 100,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        };

        assert!(calculator.calculate("openai", "unknown-model", &usage).is_none());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p hermes-core -- usage_pricing`
Expected: 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/usage_pricing.rs
git commit -m "feat(core): add usage_pricing with PricingDatabase and CostCalculator

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Create rate_limit_tracker.rs — 速率限制追踪器

**Files:**
- Create: `crates/hermes-core/src/rate_limit_tracker.rs`

- [ ] **Step 1: Create the file**

```rust
//! Rate Limit Tracker — 从响应头捕获速率限制状态
//!
//! 记录限流事件并输出 JSON 格式日志。

use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Rate Limit 事件
#[derive(Debug, Clone, Serialize)]
pub struct RateLimitEvent {
    pub event: String,
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

    /// 记录限流事件并输出 JSON 日志到 stdout
    pub fn record(&self, provider: &str, retry_after: u64) {
        let event = RateLimitEvent {
            event: "rate_limited".to_string(),
            provider: provider.to_string(),
            retry_after_secs: retry_after,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
        };

        println!("{}", serde_json::to_string(&event).unwrap());
    }
}

impl Default for RateLimitTracker {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/rate_limit_tracker.rs
git commit -m "feat(core): add rate_limit_tracker module

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Update display.rs — 添加 show_usage 方法

**Files:**
- Modify: `crates/hermes-core/src/display.rs:29`

- [ ] **Step 1: Add show_usage to trait**

Change the `DisplayHandler` trait to add:

```rust
    /// 显示 LLM 调用用量
    fn show_usage(&self, insights: &crate::insights::SessionInsights);
```

- [ ] **Step 2: Add NoopDisplay implementation**

Add to `impl DisplayHandler for NoopDisplay`:

```rust
    fn show_usage(&self, _insights: &crate::insights::SessionInsights) {}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/src/display.rs
git commit -m "feat(core): add show_usage method to DisplayHandler trait

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Update lib.rs — 导出新模块

**Files:**
- Modify: `crates/hermes-core/src/lib.rs`

- [ ] **Step 1: Add module declarations**

Add after existing module declarations:

```rust
pub mod insights;
pub mod usage_pricing;
pub mod rate_limit_tracker;
```

- [ ] **Step 2: Add exports**

Add to the `pub use` section:

```rust
pub use insights::{InsightsTracker, InMemoryInsightsTracker, SessionInsights, ToolCallRecord};
pub use usage_pricing::{PricingDatabase, PricingTier, CostCalculator};
pub use rate_limit_tracker::RateLimitTracker;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/src/lib.rs
git commit -m "feat(core): export insights, usage_pricing, rate_limit_tracker modules

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: Integrate into Agent — 集成 trackers

**Files:**
- Modify: `crates/hermes-core/src/agent.rs`

- [ ] **Step 1: Add new fields to Agent struct**

Add after `trajectory_saver`:

```rust
    // Insights tracker
    insights_tracker: Option<Arc<dyn InsightsTracker>>,
    // Rate limit tracker
    rate_limit_tracker: Option<Arc<RateLimitTracker>>,
```

- [ ] **Step 2: Update Agent::new() signature**

Add parameters:

```rust
    pub fn new(
        // ... existing params ...
        insights_tracker: Option<Arc<dyn InsightsTracker>>,
        rate_limit_tracker: Option<Arc<RateLimitTracker>>,
    ) -> Self {
        // ... add to struct initialization ...
        insights_tracker,
        rate_limit_tracker,
    }
```

- [ ] **Step 3: Update Agent::new_with_nudge_disabled()**

Add parameters and pass through:

```rust
    pub fn new_with_nudge_disabled(
        // ... existing params ...
        insights_tracker: Option<Arc<dyn InsightsTracker>>,
        rate_limit_tracker: Option<Arc<RateLimitTracker>>,
    ) -> Self {
        Self::new(
            // ... existing args ...
            insights_tracker,
            rate_limit_tracker,
        )
    }
```

- [ ] **Step 4: Add tool call tracking in run_conversation**

In the tool execution loop, after a successful tool call:

```rust
if let Some(tracker) = &self.insights_tracker {
    let record = ToolCallRecord {
        tool_name: call.name.clone(),
        started_at: started,
        duration_ms: elapsed.as_millis() as u64,
        success: true,
        error: None,
    };
    tracker.record_tool_call(record);
}
```

- [ ] **Step 5: Add usage tracking after LLM response**

After receiving LLM response (in the Stop match arm, after saving session):

```rust
if let Some(tracker) = &self.insights_tracker {
    if let Some(usage) = &response.usage {
        let calculator = CostCalculator::new(&PricingDatabase::new());
        let cost = calculator
            .calculate(&self.config.model.split('/').next().unwrap_or("unknown"), &self.config.model, usage)
            .unwrap_or(0.0);
        tracker.record_usage(usage, cost);

        // Show usage via display handler
        if let Some(display) = &self.display_handler {
            display.show_usage(&tracker.get_insights());
            display.flush();
        }
    }
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add crates/hermes-core/src/agent.rs
git commit -m "feat(core): integrate insights_tracker and rate_limit_tracker into Agent

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Update CliDisplay — 实现 show_usage

**Files:**
- Modify: `crates/hermes-cli/src/display.rs`

- [ ] **Step 1: Add show_usage implementation**

Add to the `impl DisplayHandler for CliDisplay`:

```rust
    fn show_usage(&self, insights: &hermes_core::SessionInsights) {
        let input_k = insights.input_tokens as f64 / 1000.0;
        let output_k = insights.output_tokens as f64 / 1000.0;
        let cost = insights.estimated_cost_usd;

        // Count tool calls by name
        let mut tool_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for call in &insights.tool_calls {
            *tool_counts.entry(call.tool_name.as_str()).or_insert(0) += 1;
        }

        let tools_str = if tool_counts.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = tool_counts
                .iter()
                .map(|(name, count)| format!("{}({})", name, count))
                .collect();
            format!(" | {}", parts.join(", "))
        };

        eprint!(
            "\r\x1b[K[Tokens: {:.1}K/{:.1}K | Cost: ${:.4}{}]",
            input_k, output_k, cost, tools_str
        );
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-cli`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-cli/src/display.rs
git commit -m "feat(cli): implement show_usage in CliDisplay

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Update chat.rs — 传入 trackers

**Files:**
- Modify: `crates/hermes-cli/src/chat.rs`

- [ ] **Step 1: Update Agent::new() call**

Add the new parameters:

```rust
use hermes_core::{InsightsTracker, InMemoryInsightsTracker, PricingDatabase, CostCalculator, RateLimitTracker};

let insights_tracker: Option<Arc<dyn InsightsTracker>> = Some(Arc::new(
    InMemoryInsightsTracker::new(&session_id, "openai", &model)
));
let rate_limit_tracker: Option<Arc<RateLimitTracker>> = Some(Arc::new(RateLimitTracker::new()));

let agent = Arc::new(Agent::new(
    provider,
    tool_registry,
    session_store.clone(),
    agent_config,
    nudge_config,
    display_handler,
    title_generator,
    trajectory_saver,
    insights_tracker,  // new
    rate_limit_tracker,  // new
));
```

- [ ] **Step 2: Update Agent::new_with_nudge_disabled() call in gateway.rs**

Also update `crates/hermes-cli/src/handlers/gateway.rs` with the new parameters (pass None for insights_tracker and rate_limit_tracker since gateway doesn't need them).

- [ ] **Step 3: Verify compilation**

Run: `cargo check --all`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-cli/src/chat.rs crates/hermes-cli/src/handlers/gateway.rs
git commit -m "feat(cli): wire up insights_tracker and rate_limit_tracker in chat

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 9: Add integration tests

**Files:**
- Create: `crates/hermes-core/tests/test_insights.rs`

- [ ] **Step 1: Create test file**

```rust
//! Integration tests for insights module

use hermes_core::{
    insights::{InMemoryInsightsTracker, InsightsTracker, SessionInsights, ToolCallRecord},
    usage_pricing::{CostCalculator, PricingDatabase},
    Usage,
};

#[test]
fn test_insights_tracker_records_tool_calls() {
    let tracker = InMemoryInsightsTracker::new("session1", "openai", "gpt-4o");

    tracker.record_tool_call(ToolCallRecord {
        tool_name: "ReadFile".to_string(),
        started_at: 1000.0,
        duration_ms: 50,
        success: true,
        error: None,
    });

    let insights = tracker.get_insights();
    assert_eq!(insights.tool_calls.len(), 1);
    assert_eq!(insights.tool_calls[0].tool_name, "ReadFile");
}

#[test]
fn test_insights_tracker_records_usage() {
    let tracker = InMemoryInsightsTracker::new("session1", "openai", "gpt-4o");

    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 2000,
        cache_read_tokens: Some(500),
        cache_write_tokens: Some(100),
        reasoning_tokens: None,
    };

    tracker.record_usage(&usage, 0.036625);

    let insights = tracker.get_insights();
    assert_eq!(insights.input_tokens, 1000);
    assert_eq!(insights.output_tokens, 2000);
    assert_eq!(insights.estimated_cost_usd, 0.036625);
}

#[test]
fn test_cost_calculator() {
    let pricing = PricingDatabase::new();
    let calculator = CostCalculator::new(&pricing);

    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 2000,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
    };

    let cost = calculator.calculate("openai", "gpt-4o-mini", &usage);
    assert!(cost.is_some());
    // 1000/1M * $0.15 + 2000/1M * $0.60 = $0.00135
    assert!((cost.unwrap() - 0.00135).abs() < 0.0001);
}

#[test]
fn test_session_insights_new() {
    let insights = SessionInsights::new("sess1", "anthropic", "claude-3-5-sonnet-20241022");
    assert_eq!(insights.session_id, "sess1");
    assert_eq!(insights.provider, "anthropic");
    assert_eq!(insights.model, "claude-3-5-sonnet-20241022");
    assert_eq!(insights.input_tokens, 0);
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p hermes-core --test test_insights`
Expected: 4 tests pass

- [ ] **Step 3: Run all tests**

Run: `cargo test --all`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/tests/test_insights.rs
git commit -m "test(core): add integration tests for insights module

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage:**
- [x] insights.rs: SessionInsights, ToolCallRecord, InsightsTracker trait — Task 1
- [x] usage_pricing.rs: PricingDatabase, PricingTier, CostCalculator — Task 2
- [x] rate_limit_tracker.rs: RateLimitTracker — Task 3
- [x] display.rs show_usage extension — Task 4
- [x] lib.rs exports — Task 5
- [x] Agent integration — Task 6
- [x] CliDisplay show_usage — Task 7
- [x] chat.rs wiring — Task 8
- [x] Integration tests — Task 9

**2. Placeholder scan:**
No "TBD", "TODO", or incomplete sections found.

**3. Type consistency:**
- `InsightsTracker::record_tool_call` takes `ToolCallRecord`
- `InsightsTracker::record_usage` takes `&Usage` and `f64` cost
- `CostCalculator::calculate` returns `Option<f64>`
- All signatures match between tasks

**All spec requirements covered. Plan complete.**
