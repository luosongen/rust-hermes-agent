# Hermes Agent 功能完整实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成 rust-hermes-agent 与 NousResearch/hermes-agent (Python) 的功能对齐

**Architecture:** 多阶段独立开发，每个阶段产生可测试的交付物

**Tech Stack:** Rust ( Tokio async runtime, bollard for Docker, ssh2 for SSH), SQLite/FTS5

---

## 项目现状分析

### 已高度完善的功能 ✅

| 阶段 | 功能 | 状态 |
|------|------|------|
| **阶段1.1** | ContextCompressor | ✅ 完整实现 |
| **阶段1.1** | PromptCaching (CacheDispatcher) | ✅ 完整实现 |
| **阶段1.1** | ContextEngine trait | ✅ 已定义 |
| **阶段1.2** | LocalEnvironment | ✅ 完整实现 |
| **阶段1.2** | DockerEnvironment | ✅ 完整实现 |
| **阶段1.2** | SSHEnvironment | ✅ 完整实现 |
| **阶段1.2** | EnvironmentManager | ✅ 完整实现 |

### 待实现功能 📋

| 阶段 | 功能 | 优先级 | 工作量 |
|------|------|--------|--------|
| **1.1** | ContextPressureMonitor (上下文压力监控) | 中 | 小 |
| **2.1** | Cron Scheduler (定时调度系统) | 高 | 中 |
| **2.2** | Terminal UI 增强 | 高 | 中 |
| **2.3** | MCP Client | 中 | 中 |
| **3.1** | Skills Hub 完整实现 | 高 | 大 |
| **4.1** | 消息平台适配器集群 (14个平台) | 高 | 大 |
| **5.1** | Sub-Agent 委托 | 中 | 中 |
| **5.2** | Home Assistant 集成 | 低 | 中 |
| **5.3** | 备份/导入系统 | 中 | 小 |
| **5.4** | 多实例 Profiles | 中 | 小 |
| **5.5** | 自我改进学习循环 | 低 | 大 |
| **5.6** | 皮肤/主题系统 | 低 | 小 |
| **5.7** | RL 训练工具 | 低 | 中 |

---

# 阶段1：基础设施完善

## 1.1 上下文压力监控 (ContextPressureMonitor)

**现状**: 设计文档中有提及，但代码中未实现
**目标**: 实现 tiered context pressure 警告系统

### 文件变更清单
- Create: `crates/hermes-core/src/context_pressure_monitor.rs`
- Modify: `crates/hermes-core/src/lib.rs` (导出新模块)
- Modify: `crates/hermes-core/src/agent.rs` (集成压力监控)

### 任务分解

#### Task 1.1.1: 实现 ContextPressureMonitor

**Files:**
- Create: `crates/hermes-core/src/context_pressure_monitor.rs`
- Test: `crates/hermes-core/tests/test_context_pressure.rs`

- [ ] **Step 1: 编写测试**

```rust
// crates/hermes-core/tests/test_context_pressure.rs
#[cfg(test)]
mod tests {
    use hermes_core::context_pressure_monitor::{ContextPressureMonitor, PressureLevel};

    #[tokio::test]
    async fn test_pressure_levels() {
        let monitor = ContextPressureMonitor::new(100_000, 50_000, 75_000);
        // 0-50%: Normal
        assert_eq!(monitor.get_pressure_level(0), PressureLevel::Normal);
        assert_eq!(monitor.get_pressure_level(49_999), PressureLevel::Normal);
        // 50-75%: Moderate
        assert_eq!(monitor.get_pressure_level(50_000), PressureLevel::Moderate);
        assert_eq!(monitor.get_pressure_level(74_999), PressureLevel::Moderate);
        // 75-90%: High
        assert_eq!(monitor.get_pressure_level(75_000), PressureLevel::High);
        assert_eq!(monitor.get_pressure_level(89_999), PressureLevel::High);
        // 90%+: Critical
        assert_eq!(monitor.get_pressure_level(90_000), PressureLevel::Critical);
    }

    #[tokio::test]
    async fn test_warning_message_generation() {
        let monitor = ContextPressureMonitor::new(100_000, 50_000, 75_000);
        let warning = monitor.get_warning_message(85_000);
        assert!(warning.contains("high"));
        assert!(warning.contains("85"));
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p hermes-core test_pressure_levels -- --nocapture`
Expected: FAIL - module not found

- [ ] **Step 3: 实现 ContextPressureMonitor**

```rust
// crates/hermes-core/src/context_pressure_monitor.rs

use serde::{Deserialize, Serialize};

/// 压力等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureLevel {
    Normal,    // < 50%
    Moderate,  // 50-75%
    High,      // 75-90%
    Critical,  // >= 90%
}

/// 上下文压力监控器
///
/// 监控上下文窗口使用情况，在不同阈值时提供警告。
pub struct ContextPressureMonitor {
    /// 上下文窗口总量
    context_length: usize,
    /// 温和警告阈值 (50%)
    moderate_threshold: usize,
    /// 高度警告阈值 (75%)
    high_threshold: usize,
    /// 严重警告阈值 (90%)
    critical_threshold: usize,
}

impl ContextPressureMonitor {
    pub fn new(context_length: usize) -> Self {
        Self {
            context_length,
            moderate_threshold: context_length * 50 / 100,
            high_threshold: context_length * 75 / 100,
            critical_threshold: context_length * 90 / 100,
        }
    }

    pub fn with_custom_thresholds(
        context_length: usize,
        moderate: usize,
        high: usize,
    ) -> Self {
        Self {
            context_length,
            moderate_threshold: moderate,
            high_threshold: high,
            critical_threshold: context_length * 90 / 100,
        }
    }

    /// 获取当前压力等级
    pub fn get_pressure_level(&self, current_tokens: usize) -> PressureLevel {
        let ratio = current_tokens as f64 / self.context_length as f64;
        if ratio >= 0.90 {
            PressureLevel::Critical
        } else if ratio >= 0.75 {
            PressureLevel::High
        } else if ratio >= 0.50 {
            PressureLevel::Moderate
        } else {
            PressureLevel::Normal
        }
    }

    /// 获取警告消息
    pub fn get_warning_message(&self, current_tokens: usize) -> String {
        let level = self.get_pressure_level(current_tokens);
        let percentage = (current_tokens as f64 / self.context_length as f64 * 100.0) as usize;

        match level {
            PressureLevel::Normal => String::new(),
            PressureLevel::Moderate => format!(
                "⚠️ Context at {}% — Consider preparing to compress if conversation gets longer.",
                percentage
            ),
            PressureLevel::High => format!(
                "🔶 High context pressure ({}%) — Compression recommended.",
                percentage
            ),
            PressureLevel::Critical => format!(
                "🔴 Critical context pressure ({}%) — Compression will occur soon.",
                percentage
            ),
        }
    }

    /// 检查是否需要主动压缩
    pub fn should_compress(&self, current_tokens: usize) -> bool {
        self.get_pressure_level(current_tokens) == PressureLevel::Critical
    }

    /// 获取当前使用率
    pub fn usage_ratio(&self, current_tokens: usize) -> f64 {
        current_tokens as f64 / self.context_length as f64
    }
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p hermes-core test_pressure_levels -- --nocapture`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-core/src/context_pressure_monitor.rs crates/hermes-core/tests/test_context_pressure.rs
git commit -m "feat(core): add ContextPressureMonitor for tiered context warnings"
```

#### Task 1.1.2: 集成到 Agent 主循环

**Files:**
- Modify: `crates/hermes-core/src/lib.rs`
- Modify: `crates/hermes-core/src/agent.rs`

- [ ] **Step 1: 更新 lib.rs 导出**

```rust
// crates/hermes-core/src/lib.rs
pub mod context_pressure_monitor;
pub use context_pressure_monitor::{ContextPressureMonitor, PressureLevel};
```

- [ ] **Step 2: 在 Agent 中集成压力监控**

修改 `agent.rs` 中的主循环，在每次 LLM 调用前检查上下文压力：

```rust
// 在 run_conversation 方法中，loop 开始处添加压力检查
loop {
    // ... iterations check ...

    // Context pressure monitoring
    let prompt_tokens = messages.iter().map(|m| self.provider.estimate_tokens(&m.content_text(), &model_id)).sum();
    let monitor = ContextPressureMonitor::new(self.provider.context_length(&model_id).unwrap_or(4096));

    if let Some(display) = &self.display_handler {
        if let Some(warning) = monitor.get_warning_message(prompt_tokens) {
            display.show_warning(&warning);
        }
    }

    // 主动压缩检查
    if monitor.should_compress(prompt_tokens) && iterations == 0 {
        // 第一次迭代就达到临界值，先压缩再继续
        let mut compressor = crate::ContextCompressor::new(
            self.provider.clone(),
            self.config.model.clone(),
            self.provider.context_length(&model_id).unwrap_or(4096),
        );
        if let Ok(compressed) = compressor.compress(messages.clone(), None, None).await {
            messages = compressed;
        }
    }

    // ... rest of loop ...
}
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p hermes-core`
Expected: All tests pass

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-core/src/lib.rs crates/hermes-core/src/agent.rs
git commit -m "feat(core): integrate ContextPressureMonitor into Agent loop"
```

### 验收标准
- [ ] ContextPressureMonitor 正确计算压力等级
- [ ] Agent 在上下文达到 90% 时主动触发压缩
- [ ] Display handler 能显示警告消息
- [ ] 测试覆盖率 > 80%

---

## 1.2 执行环境扩展 (已完成 ✅)

DockerEnvironment 和 SSHEnvironment 已完整实现，无需额外工作。

**验证:**
```bash
cargo test -p hermes-environment
```

---

# 阶段2：核心功能增强

## 2.1 Cron Scheduler (定时调度系统)

**目标**: 实现完整的 cron 调度系统，支持自然语言配置和后台监控

### 文件变更清单

- Create: `crates/hermes-cron/Cargo.toml`
- Create: `crates/hermes-cron/src/lib.rs`
- Create: `crates/hermes-cron/src/scheduler.rs`
- Create: `crates/hermes-cron/src/natural_language.rs`
- Create: `crates/hermes-cron/src/job.rs`
- Create: `crates/hermes-cron/src/watch.rs`
- Create: `crates/hermes-cron/tests/test_scheduler.rs`
- Modify: `crates/hermes-cli/src/main.rs` (添加 cron 命令)

### 任务分解

#### Task 2.1.1: 创建 hermes-cron crate

**Files:**
- Create: `crates/hermes-cron/Cargo.toml`
- Create: `crates/hermes-cron/src/lib.rs`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
# crates/hermes-cron/Cargo.toml
[package]
name = "hermes-cron"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true

[dependencies]
tokio = { workspace = true, features = ["sync", "time", "macros"] }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
cron = { workspace = true }
async-trait = { workspace = true }
tokio-cron-scheduler = "0.10"
```

- [ ] **Step 2: 创建 lib.rs**

```rust
// crates/hermes-cron/src/lib.rs
pub mod scheduler;
pub mod natural_language;
pub mod job;
pub mod watch;

pub use scheduler::{CronScheduler, CronError};
pub use job::{ScheduledJob, JobId, JobCommand, Schedule};
pub use natural_language::NaturalLanguageParser;
pub use watch::{WatchPatternMonitor, WatchPattern, WatchEvent};
```

- [ ] **Step 3: 运行 cargo check**

Run: `cargo check -p hermes-cron`
Expected: Compilation successful

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-cron/
git commit -m "feat(cron): initial hermes-cron crate"
```

#### Task 2.1.2: 实现 Schedule 和 JobCommand traits

**Files:**
- Create: `crates/hermes-cron/src/job.rs`
- Create: `crates/hermes-cron/tests/test_job.rs`

- [ ] **Step 1: 编写测试**

```rust
// crates/hermes-cron/tests/test_job.rs
#[cfg(test)]
mod tests {
    use hermes_cron::{JobId, Schedule, CronExpression};

    #[test]
    fn test_cron_expression_parse() {
        let expr = CronExpression::parse("0 9 * * *").unwrap();
        assert_eq!(expr.minute, 0);
        assert_eq!(expr.hour, 9);
    }

    #[test]
    fn test_schedule_variants() {
        let cron = Schedule::Cron(CronExpression::parse("*/5 * * * *").unwrap());
        assert!(matches!(cron, Schedule::Cron(_)));
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p hermes-cron test_job -- --nocapture`
Expected: FAIL

- [ ] **Step 3: 实现 job.rs**

```rust
// crates/hermes-cron/src/job.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Job 唯一标识符
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct JobId(pub String);

impl JobId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Cron 表达式
#[derive(Debug, Clone)]
pub struct CronExpression {
    pub minute: u8,
    pub hour: u8,
    pub day_of_month: u8,
    pub month: u8,
    pub day_of_week: u8,
}

impl CronExpression {
    /// 解析 cron 表达式字符串
    pub fn parse(s: &str) -> Result<Self, CronError> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(CronError::InvalidExpression("Expected 5 fields".into()));
        }

        Ok(Self {
            minute: parse_cron_field(parts[0], 0, 59)?,
            hour: parse_cron_field(parts[1], 0, 23)?,
            day_of_month: parse_cron_field(parts[2], 1, 31)?,
            month: parse_cron_field(parts[3], 1, 12)?,
            day_of_week: parse_cron_field(parts[4], 0, 6)?,
        })
    }
}

fn parse_cron_field(s: &str, min: u8, max: u8) -> Result<u8, CronError> {
    // 处理 "*"
    if s == "*" {
        return Ok(min);
    }
    // 处理 "*/n" 格式
    if let Some(n) = s.strip_prefix("*/") {
        let divisor: u8 = n.parse().map_err(|_| CronError::InvalidExpression(s.into()))?;
        return Ok(min); // 简化处理
    }
    // 处理具体数字
    let val: u8 = s.parse().map_err(|_| CronError::InvalidExpression(s.into()))?;
    if val < min || val > max {
        return Err(CronError::InvalidExpression(format!("{} out of range {}-{}", val, min, max)));
    }
    Ok(val)
}

/// 调度计划
#[derive(Debug, Clone)]
pub enum Schedule {
    Cron(CronExpression),
    NaturalLanguage(String),
    Interval(Duration),
}

/// 定时任务
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub id: JobId,
    pub name: String,
    pub schedule: Schedule,
    pub command: Box<dyn JobCommand>,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

impl ScheduledJob {
    pub fn new(id: JobId, name: String, schedule: Schedule, command: Box<dyn JobCommand>) -> Self {
        Self {
            id,
            name,
            schedule,
            command,
            enabled: true,
            last_run: None,
            next_run: None,
        }
    }
}

/// Job 命令特征
#[async_trait::async_trait]
pub trait JobCommand: Send + Sync {
    async fn execute(&self, context: &JobContext) -> Result<JobOutput, JobError>;
}

/// Job 执行上下文
#[derive(Debug, Clone)]
pub struct JobContext {
    pub job_id: JobId,
    pub job_name: String,
    pub started_at: DateTime<Utc>,
}

/// Job 执行输出
#[derive(Debug, Clone)]
pub struct JobOutput {
    pub success: bool,
    pub message: String,
    pub duration_ms: u64,
}

/// Job 相关错误
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("Job not found: {0}")]
    NotFound(JobId),
}

#[derive(Debug, thiserror::Error)]
pub enum CronError {
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p hermes-cron test_job -- --nocapture`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-cron/src/job.rs crates/hermes-cron/tests/test_job.rs
git commit -m "feat(cron): add JobId, Schedule, CronExpression types"
```

#### Task 2.1.3: 实现自然语言解析器

**Files:**
- Create: `crates/hermes-cron/src/natural_language.rs`
- Create: `crates/hermes-cron/tests/test_natural_language.rs`

- [ ] **Step 1: 编写测试**

```rust
// crates/hermes-cron/tests/test_natural_language.rs
#[cfg(test)]
mod tests {
    use hermes_cron::NaturalLanguageParser;

    #[test]
    fn test_chinese_daily_at_9am() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse("每天早上9点");
        assert_eq!(result, Some("0 9 * * *".to_string()));
    }

    #[test]
    fn test_chinese_every_5_minutes() {
        let parser = NaturalLanguageParser::new();
        let result = parser.parse("每隔5分钟");
        assert_eq!(result, Some("*/5 * * * *".to_string()));
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p hermes-cron test_natural_language -- --nocapture`
Expected: FAIL

- [ ] **Step 3: 实现 natural_language.rs**

```rust
// crates/hermes-cron/src/natural_language.rs

use crate::{CronExpression, CronError};

/// 自然语言到 Cron 表达式的解析器
pub struct NaturalLanguageParser {
    // 中英文模式匹配规则
}

impl NaturalLanguageParser {
    pub fn new() -> Self {
        Self {}
    }

    /// 将自然语言解析为 cron 表达式
    pub fn parse(&self, input: &str) -> Option<String> {
        let input = input.trim();

        // 中文模式
        if let Some(expr) = self.parse_chinese(input) {
            return Some(expr);
        }

        // 英文模式
        if let Some(expr) = self.parse_english(input) {
            return Some(expr);
        }

        None
    }

    fn parse_chinese(&self, input: &str) -> Option<String> {
        // 每天早上9点 -> "0 9 * * *"
        if input.contains("每天") && input.contains("早上") {
            if let Some(hour) = extract_hour(input, "早上", "点") {
                return Some(format!("0 {} * * *", hour));
            }
        }

        // 每隔N分钟
        if input.contains("每隔") && input.contains("分钟") {
            if let Some(mins) = extract_number(input, "每隔", "分钟") {
                return Some(format!("*/{} * * * *", mins));
            }
        }

        // 工作日每半小时
        if input.contains("工作日") && input.contains("半小时") {
            return Some("*/30 9-18 * * 1-5".to_string());
        }

        // 每周一早上10点
        if input.contains("每周") && input.contains("早上") {
            let day = day_of_week_to_number(input)?;
            if let Some(hour) = extract_hour(input, "早上", "点") {
                return Some(format!("0 {} * * {}", day, hour));
            }
        }

        None
    }

    fn parse_english(&self, input: &str) -> Option<String> {
        let input = input.to_lowercase();

        // "every 5 minutes"
        if input.contains("every") && input.contains("minute") {
            if let Some(n) = extract_number_str(&input, "every ", " minute") {
                return Some(format!("*/{} * * * *", n));
            }
        }

        // "daily at 9am"
        if input.contains("daily") && input.contains("am") {
            if let Some(hour) = extract_hour_str(&input, " at ", "am") {
                return Some(format!("0 {} * * *", hour));
            }
        }

        None
    }
}

impl Default for NaturalLanguageParser {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_hour(input: &str, prefix: &str, suffix: &str) -> Option<u8> {
    let after_prefix = input.find(prefix)? + prefix.len();
    let before_suffix = input[..after_prefix].find('点')?;
    let hour_str = &input[after_prefix..before_suffix];
    hour_str.parse().ok()
}

fn extract_hour_str(input: &str, prefix: &str, suffix: &str) -> Option<u8> {
    let after_prefix = input.find(prefix)? + prefix.len();
    let before_suffix = input[..after_prefix].find(suffix)?;
    let hour_str = &input[after_prefix..before_suffix];
    hour_str.trim().parse().ok()
}

fn extract_number(input: &str, prefix: &str, suffix: &str) -> Option<u8> {
    let after_prefix = input.find(prefix)? + prefix.len();
    let before_suffix = input[after_prefix..].find(suffix)?;
    let num_str = &input[after_prefix..after_prefix + before_suffix];
    num_str.trim().parse().ok()
}

fn extract_number_str(input: &str, prefix: &str, suffix: &str) -> Option<String> {
    let after_prefix = input.find(prefix)? + prefix.len();
    let before_suffix = input[after_prefix..].find(suffix)?;
    Some(input[after_prefix..after_prefix + before_suffix].trim().to_string())
}

fn day_of_week_to_number(input: &str) -> Option<u8> {
    let day_names = [
        ("一", 1), ("二", 2), ("三", 3), ("四", 4),
        ("五", 5), ("六", 6), ("日", 7),
    ];

    for (name, num) in day_names {
        if input.contains(name) {
            return Some(num);
        }
    }
    None
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p hermes-cron test_natural_language -- --nocapture`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-cron/src/natural_language.rs crates/hermes-cron/tests/test_natural_language.rs
git commit -m "feat(cron): add NaturalLanguageParser for cron expressions"
```

#### Task 2.1.4: 实现 CronScheduler

**Files:**
- Create: `crates/hermes-cron/src/scheduler.rs`
- Create: `crates/hermes-cron/tests/test_scheduler.rs`

- [ ] **Step 1: 编写测试**

```rust
// crates/hermes-cron/tests/test_scheduler.rs
#[cfg(test)]
mod tests {
    use hermes_cron::{CronScheduler, JobId, Schedule, CronExpression};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_add_job() {
        let scheduler = Arc::new(CronScheduler::new());
        let job_id = JobId::new("test-job");
        // Add job test...
        let jobs = scheduler.list_jobs().await;
        assert!(!jobs.is_empty());
    }
}
```

- [ ] **Step 2: 运行测试验证失败**

Run: `cargo test -p hermes-cron test_scheduler -- --nocapture`
Expected: FAIL

- [ ] **Step 3: 实现 scheduler.rs**

```rust
// crates/hermes-cron/src/scheduler.rs

use crate::{CronError, JobCommand, JobContext, JobError, JobId, JobOutput, ScheduledJob, Schedule};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_cron_scheduler::{Scheduler, Job};

/// Cron 调度器
pub struct CronScheduler {
    jobs: Arc<RwLock<HashMap<JobId, ScheduledJob>>>,
    runtime: Scheduler,
}

impl CronScheduler {
    pub async fn new() -> Result<Self, CronError> {
        let runtime = Scheduler::new()
            .await
            .map_err(|e| CronError::InvalidExpression(e.to_string()))?;
        Ok(Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            runtime,
        })
    }

    pub async fn add_job(&self, job: ScheduledJob) -> Result<(), CronError> {
        let job_id = job.id.clone();
        let schedule_str = match &job.schedule {
            Schedule::Cron(expr) => format!("{} {} {} {} {}",
                expr.minute, expr.hour, expr.day_of_month, expr.month, expr.day_of_week),
            _ => return Err(CronError::InvalidExpression("Only Cron schedule supported yet".into())),
        };

        let command = job.command.as_ref().clone();
        let jobs = self.jobs.clone();

        self.runtime
            .add(Job::new_async(schedule_str.as_str(), move |_uuid, _l| {
                let cmd = command.box_clone();
                let jobs_clone = jobs.clone();
                Box::pin(async move {
                    let context = JobContext {
                        job_id: JobId::new("temp"),
                        job_name: "temp".to_string(),
                        started_at: chrono::Utc::now(),
                    };
                    let _ = cmd.execute(&context).await;
                })
            }))
            .await
            .map_err(|e| CronError::InvalidExpression(e.to_string()))?;

        self.jobs.write().await.insert(job_id, job);
        Ok(())
    }

    pub async fn remove_job(&self, job_id: &JobId) -> Result<(), JobError> {
        self.jobs.write().await.remove(job_id);
        Ok(())
    }

    pub async fn list_jobs(&self) -> Vec<ScheduledJob> {
        self.jobs.read().await.values().cloned().collect()
    }

    pub async fn start(&self) -> Result<(), CronError> {
        self.runtime
            .start()
            .await
            .map_err(|e| CronError::InvalidExpression(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 4: 运行测试验证通过**

Run: `cargo test -p hermes-cron test_scheduler -- --nocapture`
Expected: PASS (或部分通过，因为涉及 async runtime)

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-cron/src/scheduler.rs crates/hermes-cron/tests/test_scheduler.rs
git commit -m "feat(cron): add CronScheduler with tokio-cron-scheduler"
```

#### Task 2.1.5: 添加 CLI 命令

**Files:**
- Modify: `crates/hermes-cli/src/main.rs`

- [ ] **Step 1: 添加 cron 子命令**

```rust
// 在 hermes-cli/src/main.rs 添加
#[derive(Clap)]
pub enum SubCommand {
    /// 定时任务管理
    Cron {
        #[clap(subcommand)]
        command: CronCommand,
    },
}

#[derive(Clap)]
pub enum CronCommand {
    /// 列出所有定时任务
    List,
    /// 添加定时任务
    Add {
        #[clap(long)]
        name: String,
        #[clap(long)]
        schedule: String,
        #[clap(long)]
        command: String,
    },
    /// 删除定时任务
    Delete {
        #[clap(long)]
        id: String,
    },
}
```

- [ ] **Step 2: 实现 cron 命令处理**

```rust
// 在 run() 方法中添加
SubCommand::Cron { command } => {
    match command {
        CronCommand::List => {
            // 调用 hermes-cron 列出任务
        }
        CronCommand::Add { name, schedule, command } => {
            // 添加新任务
        }
        CronCommand::Delete { id } => {
            // 删除任务
        }
    }
}
```

- [ ] **Step 3: 测试 CLI**

Run: `cargo run --bin hermes -- cron list`
Expected: 显示帮助或任务列表

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-cli/src/main.rs
git commit -m "feat(cli): add cron subcommand"
```

### 验收标准
- [ ] CronScheduler 正确调度任务
- [ ] NaturalLanguageParser 解析中文/英文自然语言
- [ ] CLI 命令 `hermes cron list/add/delete` 正常工作
- [ ] 测试覆盖率 > 80%

---

## 2.2 Terminal UI 增强

**目标**: 实现完整的增强终端 UI

### 文件变更清单

- Create: `crates/hermes-cli/src/ui/mod.rs`
- Create: `crates/hermes-cli/src/ui/multiline_editor.rs`
- Create: `crates/hermes-cli/src/ui/completer.rs`
- Create: `crates/hermes-cli/src/ui/streaming_output.rs`
- Create: `crates/hermes-cli/src/ui/command_history.rs`
- Modify: `crates/hermes-cli/src/repl.rs` (重构集成)

### 任务分解

#### Task 2.2.1: 实现 MultilineEditor

**Files:**
- Create: `crates/hermes-cli/src/ui/multiline_editor.rs`
- Create: `crates/hermes-cli/tests/test_multiline_editor.rs`

- [ ] **Step 1: 编写测试**

```rust
#[cfg(test)]
mod tests {
    use hermes_cli::ui::MultilineEditor;

    #[test]
    fn test_multiline_buffer() {
        let mut editor = MultilineEditor::new();
        editor.push_line("line 1");
        editor.push_line("line 2");
        assert_eq!(editor.buffer(), "line 1\nline 2");
    }
}
```

- [ ] **Step 2: 实现 MultilineEditor**

```rust
// crates/hermes-cli/src/ui/multiline_editor.rs

/// 多行编辑器
pub struct MultilineEditor {
    buffer: String,
    cursor_pos: usize,
    max_lines: usize,
    indent: String,
}

impl MultilineEditor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_pos: 0,
            max_lines: 100,
            indent: "  ".to_string(),
        }
    }

    pub fn push_line(&mut self, line: &str) {
        if !self.buffer.is_empty() {
            self.buffer.push('\n');
        }
        self.buffer.push_str(line);
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_pos = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn lines(&self) -> usize {
        self.buffer.lines().count()
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `cargo test -p hermes-cli test_multiline_editor`
Expected: PASS

#### Task 2.2.2: 实现 SlashCommandCompleter

**Files:**
- Create: `crates/hermes-cli/src/ui/completer.rs`
- Create: `crates/hermes-cli/tests/test_completer.rs`

- [ ] **Step 1: 实现 Completer trait**

```rust
// crates/hermes-cli/src/ui/completer.rs

use std::collections::HashMap;

/// 命令元数据
#[derive(Debug, Clone)]
pub struct CommandMetadata {
    pub name: String,
    pub description: String,
    pub subcommands: Vec<String>,
}

/// 斜杠命令自动补全器
pub struct SlashCommandCompleter {
    commands: HashMap<String, CommandMetadata>,
}

impl SlashCommandCompleter {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, meta: CommandMetadata) {
        self.commands.insert(name.to_string(), meta);
    }

    pub fn complete(&self, input: &str) -> Vec<String> {
        if !input.starts_with('/') {
            return vec![];
        }

        let partial = &input[1..];
        self.commands
            .keys()
            .filter(|k| k.starts_with(partial))
            .cloned()
            .collect()
    }
}
```

#### Task 2.2.3: 重构 REPL 集成 UI 组件

**Files:**
- Modify: `crates/hermes-cli/src/repl.rs`

- [ ] **Step 1: 重构 REPL**

```rust
// crates/hermes-cli/src/repl.rs

pub struct EnhancedRepl {
    editor: MultilineEditor,
    completer: SlashCommandCompleter,
    history: CommandHistory,
}

impl EnhancedRepl {
    pub fn new() -> Self {
        Self {
            editor: MultilineEditor::new(),
            completer: SlashCommandCompleter::new(),
            history: CommandHistory::load().unwrap_or_default(),
        }
    }
}
```

### 验收标准
- [ ] MultilineEditor 支持多行输入
- [ ] SlashCommandCompleter 正确补全命令
- [ ] CommandHistory 持久化
- [ ] REPL 集成所有 UI 组件

---

## 2.3 MCP Client

**目标**: 实现 MCP Client，支持连接外部 MCP 服务器

### 文件变更清单

- Create: `crates/hermes-mcp/Cargo.toml`
- Create: `crates/hermes-mcp/src/lib.rs`
- Create: `crates/hermes-mcp/src/client.rs`
- Create: `crates/hermes-mcp/src/transport.rs`
- Create: `crates/hermes-mcp/src/tools.rs`
- Create: `crates/hermes-mcp/tests/test_mcp_client.rs`

### 任务分解

#### Task 2.3.1: 创建 hermes-mcp crate

**Files:**
- Create: `crates/hermes-mcp/Cargo.toml`
- Create: `crates/hermes-mcp/src/lib.rs`

```toml
# crates/hermes-mcp/Cargo.toml
[package]
name = "hermes-mcp"
version.workspace = true
edition.workspace = true

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
```

#### Task 2.3.2: 实现 McpTransport trait

**Files:**
- Create: `crates/hermes-mcp/src/transport.rs`

```rust
// crates/hermes-mcp/src/transport.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMessage {
    pub jsonrpc: String,
    pub method: Option<String>,
    pub params: Option<serde_json::Value>,
    pub id: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn connect(&self) -> Result<(), McpError>;
    async fn send(&self, message: McpMessage) -> Result<McpMessage, McpError>;
    async fn receive(&self) -> Result<McpMessage, McpError>;
}

/// STDIO 传输（用于本地 MCP 服务器）
pub struct StdioTransport {
    command: String,
    args: Vec<String>,
}

/// HTTP 传输（用于远程 MCP 服务器）
pub struct HttpTransport {
    url: String,
    headers: std::collections::HashMap<String, String>,
}
```

#### Task 2.3.3: 实现 McpClient

**Files:**
- Create: `crates/hermes-mcp/src/client.rs`

```rust
// crates/hermes-mcp/src/client.rs

use crate::transport::{McpMessage, McpTransport, McpError};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct McpClient {
    transport: Box<dyn McpTransport>,
    request_id: AtomicU64,
}

impl McpClient {
    pub fn new(transport: Box<dyn McpTransport>) -> Self {
        Self {
            transport,
            request_id: AtomicU64::new(1),
        }
    }

    pub async fn initialize(&self) -> Result<(), McpError> {
        self.transport.connect().await?;
        let msg = McpMessage {
            jsonrpc: "2.0".to_string(),
            method: Some("initialize".to_string()),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "hermes-mcp",
                    "version": "0.1.0"
                }
            })),
            id: Some(self.request_id.fetch_add(1, Ordering::SeqCst)),
        };
        self.transport.send(msg).await?;
        self.transport.receive().await?;
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<ToolInfo>, McpError> {
        let msg = McpMessage {
            jsonrpc: "2.0".to_string(),
            method: Some("tools/list".to_string()),
            params: None,
            id: Some(self.request_id.fetch_add(1, Ordering::SeqCst)),
        };
        let response = self.transport.send(msg).await?;
        // 解析响应中的 tools 列表
        Ok(vec![])
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
```

### 验收标准
- [ ] McpTransport trait 支持 STDIO 和 HTTP
- [ ] McpClient 完成协议握手
- [ ] 能列出 MCP 服务器工具
- [ ] 测试覆盖率 > 80%

---

# 阶段3：Skills 系统

## 3.1 Skills Hub 完整实现

**目标**: 实现完整的 Skills Hub，支持从 agentskills.io 搜索/安装、管理和创建 Skill

### 文件变更清单

- Modify: `crates/hermes-skills/src/lib.rs`
- Create: `crates/hermes-skills/src/hub_client.rs`
- Create: `crates/hermes-skills/src/skill_creator.rs`
- Create: `crates/hermes-skills/src/procedural_memory.rs`
- Create: `crates/hermes-skills/tests/test_hub.rs`

### 任务分解

#### Task 3.1.1: 实现 SkillHubClient

**Files:**
- Create: `crates/hermes-skills/src/hub_client.rs`
- Create: `crates/hermes-skills/tests/test_hub_client.rs`

- [ ] **Step 1: 编写测试**

```rust
// crates/hermes-skills/tests/test_hub_client.rs
#[cfg(test)]
mod tests {
    use hermes_skills::hub_client::{SkillHubClient, HubSkillSummary};

    #[tokio::test]
    async fn test_search_skills() {
        let client = SkillHubClient::new("https://agentskills.io/api".parse().unwrap());
        // Mock 测试
    }
}
```

- [ ] **Step 2: 实现 SkillHubClient**

```rust
// crates/hermes-skills/src/hub_client.rs

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SkillHubClient {
    http_client: Client,
    base_url: url::Url,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubSkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum HubError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
}

impl SkillHubClient {
    pub fn new(base_url: url::Url) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        Self { http_client, base_url }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<HubSkillSummary>, HubError> {
        let url = self.base_url.join("/skills/search").unwrap();
        let response = self.http_client
            .get(url)
            .query(&[("q", query)])
            .send()
            .await?;
        let skills: Vec<HubSkillSummary> = response.json().await?;
        Ok(skills)
    }

    pub async fn install(&self, skill_id: &str) -> Result<InstalledSkill, HubError> {
        let url = self.base_url.join(&format!("/skills/{}", skill_id)).unwrap();
        let response = self.http_client.get(url).send().await?;
        let skill: InstalledSkill = response.json().await?;
        Ok(skill)
    }
}
```

#### Task 3.1.2: 实现 SkillCreator

**Files:**
- Create: `crates/hermes-skills/src/skill_creator.rs`

- [ ] **Step 1: 实现 SkillCreator**

```rust
// crates/hermes-skills/src/skill_creator.rs

use crate::{Skill, SkillError};
use hermes_core::LlmProvider;
use std::sync::Arc;

pub struct SkillCreator {
    llm_provider: Arc<dyn LlmProvider>,
}

impl SkillCreator {
    pub fn new(llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self { llm_provider }
    }

    pub async fn create_from_description(
        &self,
        name: &str,
        description: &str,
    ) -> Result<Skill, SkillError> {
        // 使用 LLM 生成 Skill 代码
        // 这是简化版本，完整实现需要模板系统
        Ok(Skill::new(name, description))
    }
}
```

#### Task 3.1.3: 实现 ProceduralMemory

**Files:**
- Create: `crates/hermes-skills/src/procedural_memory.rs`

- [ ] **Step 1: 实现用户画像**

```rust
// crates/hermes-skills/src/procedural_memory.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub preferences: UserPreferences,
    pub interaction_patterns: Vec<Pattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub verbose: bool,
    pub code_style: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub pattern_type: String,
    pub description: String,
}

pub struct ProceduralMemory {
    user_profiles: HashMap<String, UserProfile>,
}
```

### 验收标准
- [ ] SkillHubClient 能搜索和安装 Skills
- [ ] SkillCreator 能从描述创建 Skill
- [ ] ProceduralMemory 管理用户画像
- [ ] CLI 命令 `hermes skills search/install/create` 正常工作

---

# 阶段4：消息平台适配器集群

## 4.1 消息平台适配器

**目标**: 实现 Discord, Slack, Email 等 14 个平台适配器

### 文件变更清单

每个平台需要创建独立的 crate：
- Create: `crates/hermes-platform-discord/Cargo.toml`
- Create: `crates/hermes-platform-discord/src/lib.rs`
- Create: `crates/hermes-platform-discord/src/discord.rs`
- Create: `crates/hermes-platform-discord/tests/test_discord.rs`
- (其他平台同理)

### 任务分解 (以 Discord 为例)

#### Task 4.1.1: 创建 Discord 适配器

**Files:**
- Create: `crates/hermes-platform-discord/Cargo.toml`
- Create: `crates/hermes-platform-discord/src/lib.rs`
- Create: `crates/hermes-platform-discord/src/discord.rs`

```toml
# crates/hermes-platform-discord/Cargo.toml
[package]
name = "hermes-platform-discord"
version.workspace = true
edition.workspace = true

[dependencies]
hermes-core = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
```

```rust
// crates/hermes-platform-discord/src/discord.rs

use hermes_core::gateway::{GatewayError, InboundMessage, MessageContent, PlatformAdapter};
use async_trait::async_trait;
use axum::body::Body;
use reqwest::Client;
use std::sync::Arc;

pub struct DiscordAdapter {
    bot_token: String,
    http_client: Client,
}

impl DiscordAdapter {
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            http_client: Client::new(),
        }
    }
}

#[async_trait]
impl PlatformAdapter for DiscordAdapter {
    fn platform_id(&self) -> &str {
        "discord"
    }

    fn platform_name(&self) -> &str {
        "Discord"
    }

    fn verify_webhook(&self, request: &axum::http::Request<Body>) -> bool {
        // Discord 使用 Authorization header 验证
        true
    }

    async fn parse_inbound(&self, request: axum::http::Request<Body>) -> Result<InboundMessage, GatewayError> {
        // 解析 Discord webhook 格式
        let body = axum::body::to_bytes(request.into_body(), 10_000_000).await?;
        let json: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|e| GatewayError::Parse(e.to_string()))?;
        // 转换为标准 InboundMessage
        Ok(InboundMessage {
            platform: "discord".to_string(),
            message_id: json["id"].as_str().unwrap_or_default().to_string(),
            chat_id: json["channel_id"].as_str().unwrap_or_default().to_string(),
            sender_id: json["author"]["id"].as_str().unwrap_or_default().to_string(),
            sender_name: json["author"]["username"].as_str().map(String::from),
            content: MessageContent::Text(json["content"].as_str().unwrap_or_default().to_string()),
            timestamp: chrono::Utc::now(),
            raw: json,
        })
    }

    async fn send_response(&self, response: hermes_core::ConversationResponse, message: &InboundMessage) -> Result<(), GatewayError> {
        // 发送 Discord 消息
        Ok(())
    }
}
```

### 验收标准
- [ ] Discord 适配器实现完整 Bot 功能
- [ ] 每个平台适配器实现 verify_webhook, parse_inbound, send_response
- [ ] 集成到 hermes-gateway 动态加载
- [ ] 测试覆盖率 > 80%

---

# 阶段5：高级功能

## 5.1 Sub-Agent 委托

**目标**: 实现子 Agent 委托系统

### 文件变更清单

- Create: `crates/hermes-core/src/delegate.rs`
- Modify: `crates/hermes-core/src/agent.rs`
- Create: `crates/hermes-tools-builtin/src/delegate_tool.rs`

### 任务分解

#### Task 5.1.1: 实现 SubAgentDispatcher

```rust
// crates/hermes-core/src/delegate.rs

use crate::{Agent, AgentConfig, ToolContext};
use std::sync::Arc;

pub struct SubAgentDispatcher {
    agent_config: AgentConfig,
    max_concurrent: usize,
}

pub enum IsolationLevel {
    Full,
    Partial(Vec<String>),
    FullShare,
}

impl SubAgentDispatcher {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            agent_config: AgentConfig::default(),
            max_concurrent,
        }
    }

    pub async fn spawn(
        &self,
        task: &str,
        isolation: IsolationLevel,
    ) -> Result<String, DelegateError> {
        // 创建子 Agent
        // 根据 isolation level 处理上下文
        Ok("task_result".to_string())
    }
}
```

### 验收标准
- [ ] 子 Agent 能隔离上下文
- [ ] 支持并行工作流
- [ ] delegate_task 工具正常工作

---

## 5.2 Home Assistant 集成

**目标**: 实现完整的 Home Assistant 集成

### 文件变更清单

- Create: `crates/hermes-tools-homeassistant/Cargo.toml`
- Create: `crates/hermes-tools-homeassistant/src/lib.rs`
- Create: `crates/hermes-tools-homeassistant/src/ha_client.rs`
- Create: `crates/hermes-tools-homeassistant/src/tools.rs`

### 验收标准
- [ ] HA API 客户端实现
- [ ] ha_list_entities, ha_get_state, ha_call_service 等工具实现

---

## 5.3 备份/导入系统

**目标**: 实现配置、会话、Skills 完整迁移

### 文件变更清单

- Create: `crates/hermes-cli/src/commands/backup.rs`
- Create: `crates/hermes-cli/src/commands/import.rs`

### 验收标准
- [ ] `hermes backup --output backup.tar.gz` 正常工作
- [ ] `hermes import --input backup.tar.gz` 正常工作

---

## 5.4 多实例 Profiles

**目标**: 支持完全隔离的多实例

### 文件变更清单

- Modify: `crates/hermes-core/src/config/`
- Create: `crates/hermes-cli/src/commands/profile.rs`

### 验收标准
- [ ] `hermes profile list/create/switch/delete` 正常工作
- [ ] 各 Profile 配置完全隔离

---

## 5.5 自我改进学习循环

**目标**: 实现从经验中学习的 Agent 改进机制

### 文件变更清单

- Create: `crates/hermes-core/src/trajectory.rs` (已存在，需增强)
- Create: `crates/hermes-core/src/learning.rs`
- Create: `crates/hermes-core/src/user_model.rs`

### 验收标准
- [ ] TrajectoryLogger 记录决策轨迹
- [ ] LearningAnalyzer 分析成功/失败模式
- [ ] UserModeler 建立用户模型

---

## 5.6 皮肤/主题系统

**目标**: 实现 YAML 可配置的皮肤/主题系统

### 文件变更清单

- Create: `crates/hermes-skin/Cargo.toml`
- Create: `crates/hermes-skin/src/lib.rs`
- Create: `crates/hermes-skin/src/engine.rs`

### 验收标准
- [ ] SkinEngine 加载 YAML 皮肤
- [ ] 内置皮肤 (kawaii, ares, mono, slate) 可用
- [ ] CLI 命令 `hermes skin list/set/preview` 正常工作

---

## 5.7 RL 训练工具

**目标**: 实现强化学习训练工具

### 文件变更清单

- Create: `crates/hermes-tools-rl/Cargo.toml`
- Create: `crates/hermes-tools-rl/src/lib.rs`
- Create: `crates/hermes-tools-rl/src/rl_client.rs`
- Create: `crates/hermes-tools-rl/src/tools.rs`

### 验收标准
- [ ] RL API 客户端实现
- [ ] rl_list_environments, rl_start_training 等工具实现

---

# 执行方式选择

**Plan complete and saved to `docs/superpowers/plans/2026-04-25-hermes-agent-full-implementation-plan.md`.**

由于这是一个超大型项目（12周，18个新crate），建议采用**分阶段子代理执行**方式：

1. **阶段1** 可以快速完成（主要是完善现有代码）
2. **阶段2-5** 每个阶段单独执行

**Two execution options:**

**1. Subagent-Driven (recommended)** - 我 dispatch 一个 subagent 按任务执行，定期 review，快速迭代

**2. Inline Execution** - 在此 session 中使用 executing-plans 批量执行，带检查点

**Which approach?**
