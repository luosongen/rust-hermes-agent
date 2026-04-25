//! Hermes Cron — 定时调度系统
//!
//! 提供完整的 cron 调度功能，支持：
//! - 标准 cron 表达式解析
//! - 自然语言解析（中文/英文）
//! - 后台进程监控 (WatchPattern)
//! - 定时任务管理
//!
//! ## 主要类型
//! - **CronScheduler** — 核心调度器
//! - **ScheduledJob** — 定时任务
//! - **NaturalLanguageParser** — 自然语言转 cron
//! - **WatchPattern** — 文件/进程监控模式

pub mod job;
pub mod scheduler;
pub mod natural_language;
pub mod watch;

pub use job::{CronExpression, JobCommand, JobContext, JobError, JobId, JobOutput, Schedule, ScheduledJob};
pub use scheduler::{CronError, CronScheduler};
pub use natural_language::NaturalLanguageParser;
pub use watch::{WatchEvent, WatchEventType, WatchAction, WatchPattern};
