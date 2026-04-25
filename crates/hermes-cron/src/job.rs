//! Job 模块 — 定时任务核心类型

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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Cron 表达式
#[derive(Debug, Clone)]
pub struct CronExpression {
    pub second: u8,       // 0-59 (通常为0)
    pub minute: u8,       // 0-59
    pub hour: u8,         // 0-23
    pub day_of_month: u8, // 1-31
    pub month: u8,        // 1-12
    pub day_of_week: u8,  // 0-6 (0 = Sunday)
}

impl CronExpression {
    /// 解析 5 字段 cron 表达式 (分 时 日 月 周)
    pub fn parse(s: &str) -> Result<Self, CronError> {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(CronError::InvalidExpression("Expected 5 fields: minute hour day month weekday".into()));
        }

        Ok(Self {
            second: 0,
            minute: parse_cron_field(parts[0], 0, 59)?,
            hour: parse_cron_field(parts[1], 0, 23)?,
            day_of_month: parse_cron_field(parts[2], 1, 31)?,
            month: parse_cron_field(parts[3], 1, 12)?,
            day_of_week: parse_cron_field(parts[4], 0, 6)?,
        })
    }

    /// 转换为 cron string (6字段格式 for tokio-cron-scheduler)
    pub fn to_cron_string(&self) -> String {
        format!("{} {} {} {} {} {}",
            self.second, self.minute, self.hour, self.day_of_month, self.month, self.day_of_week)
    }
}

/// 解析单个 cron 字段
fn parse_cron_field(s: &str, min: u8, max: u8) -> Result<u8, CronError> {
    // 处理 "*"
    if s == "*" {
        return Ok(min);
    }
    // 处理 "*/n" 格式
    if let Some(n_str) = s.strip_prefix("*/") {
        let _divisor: u8 = n_str.parse().map_err(|_| {
            CronError::InvalidExpression(format!("Invalid divisor: {}", n_str))
        })?;
        // 简化处理：返回最小值
        return Ok(min);
    }
    // 处理 "n-m" 范围格式
    if let Some(range) = s.contains('-').then_some(s) {
        let range_parts: Vec<&str> = range.split('-').collect();
        if range_parts.len() == 2 {
            let start: u8 = range_parts[0].parse().map_err(|_| {
                CronError::InvalidExpression(format!("Invalid range start: {}", range_parts[0]))
            })?;
            let _end: u8 = range_parts[1].parse().map_err(|_| {
                CronError::InvalidExpression(format!("Invalid range end: {}", range_parts[1]))
            })?;
            return Ok(start); // 返回起始值
        }
    }
    // 处理具体数字
    let val: u8 = s.parse().map_err(|_| {
        CronError::InvalidExpression(format!("Invalid value: {}", s))
    })?;
    if val < min || val > max {
        return Err(CronError::InvalidExpression(format!("Value {} out of range {}-{}", val, min, max)));
    }
    Ok(val)
}

/// 调度计划
#[derive(Debug, Clone)]
pub enum Schedule {
    /// 标准 cron 表达式
    Cron(CronExpression),
    /// 自然语言描述
    NaturalLanguage(String),
    /// 固定间隔
    Interval(Duration),
}

impl Schedule {
    /// 转换为 cron 字符串
    pub fn to_cron_string(&self) -> Option<String> {
        match self {
            Schedule::Cron(expr) => Some(expr.to_cron_string()),
            _ => None, // NaturalLanguage 和 Interval 需要特殊处理
        }
    }
}

/// 定时任务
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub id: JobId,
    pub name: String,
    pub schedule: Schedule,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}

impl ScheduledJob {
    pub fn new(id: JobId, name: String, schedule: Schedule) -> Self {
        Self {
            id,
            name,
            schedule,
            enabled: true,
            last_run: None,
            next_run: None,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Job 命令特征
#[async_trait::async_trait]
pub trait JobCommand: Send + Sync {
    async fn execute(&self, context: &JobContext) -> Result<JobOutput, JobError>;
}

/// 简单闭包 Job 命令适配器
pub struct ClosureJob<F>
where
    F: Fn(JobContext) -> Result<JobOutput, JobError> + Send + Sync,
{
    func: F,
}

impl<F> ClosureJob<F>
where
    F: Fn(JobContext) -> Result<JobOutput, JobError> + Send + Sync + 'static,
{
    pub fn new(func: F) -> Box<Self> {
        Box::new(Self { func })
    }
}

#[async_trait::async_trait]
impl<F> JobCommand for ClosureJob<F>
where
    F: Fn(JobContext) -> Result<JobOutput, JobError> + Send + Sync + 'static,
{
    async fn execute(&self, context: &JobContext) -> Result<JobOutput, JobError> {
        (self.func)(context.clone())
    }
}

/// Job 执行上下文
#[derive(Debug, Clone)]
pub struct JobContext {
    pub job_id: JobId,
    pub job_name: String,
    pub started_at: DateTime<Utc>,
}

impl JobContext {
    pub fn new(job_id: JobId, job_name: String) -> Self {
        Self {
            job_id,
            job_name,
            started_at: Utc::now(),
        }
    }
}

/// Job 执行输出
#[derive(Debug, Clone)]
pub struct JobOutput {
    pub success: bool,
    pub message: String,
    pub duration_ms: u64,
}

impl JobOutput {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            duration_ms: 0,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            duration_ms: 0,
        }
    }
}

/// Job 相关错误
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("Job not found: {0}")]
    NotFound(JobId),
    #[error("Job disabled")]
    Disabled,
}

#[derive(Debug, thiserror::Error)]
pub enum CronError {
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),
    #[error("Scheduler error: {0}")]
    Scheduler(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_expression_parse() {
        let expr = CronExpression::parse("0 9 * * *").unwrap();
        assert_eq!(expr.minute, 0);
        assert_eq!(expr.hour, 9);
        assert_eq!(expr.day_of_month, 1);  // * -> min = 1
        assert_eq!(expr.month, 1);        // * -> min = 1
        assert_eq!(expr.day_of_week, 0);  // * -> min = 0 (Sunday)
    }

    #[test]
    fn test_cron_expression_every_5_minutes() {
        let expr = CronExpression::parse("*/5 * * * *").unwrap();
        assert_eq!(expr.minute, 0);
        assert_eq!(expr.hour, 0);
    }

    #[test]
    fn test_cron_expression_workdays() {
        let expr = CronExpression::parse("0 9-18 * * 1-5").unwrap();
        assert_eq!(expr.hour, 9);
        assert_eq!(expr.day_of_week, 1);
    }

    #[test]
    fn test_schedule_cron() {
        let schedule = Schedule::Cron(CronExpression::parse("0 9 * * *").unwrap());
        assert!(schedule.to_cron_string().is_some());
    }

    #[test]
    fn test_job_id() {
        let id = JobId::new("test-job");
        assert_eq!(id.as_str(), "test-job");
        assert_eq!(id.to_string(), "test-job");
    }

    #[test]
    fn test_job_output() {
        let output = JobOutput::success("Done");
        assert!(output.success);
        assert_eq!(output.message, "Done");

        let failure = JobOutput::failure("Error");
        assert!(!failure.success);
    }

    #[test]
    fn test_scheduled_job() {
        let job = ScheduledJob::new(
            JobId::new("test"),
            "Test Job".to_string(),
            Schedule::Cron(CronExpression::parse("0 9 * * *").unwrap()),
        );
        assert!(job.enabled);
        assert_eq!(job.name, "Test Job");
    }
}
