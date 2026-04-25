//! Scheduler 模块 — Cron 调度器实现

use crate::job::{JobError, JobId, ScheduledJob, Schedule};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cron 调度器错误
#[derive(Debug, thiserror::Error)]
pub enum CronError {
    #[error("Invalid schedule: {0}")]
    InvalidSchedule(String),
    #[error("Job not found: {0}")]
    JobNotFound(JobId),
    #[error("Scheduler error: {0}")]
    Scheduler(String),
}

/// Cron 调度器
///
/// 管理定时任务的注册、调度和执行。
pub struct CronScheduler {
    jobs: Arc<RwLock<HashMap<JobId, ScheduledJob>>>,
    running: Arc<RwLock<bool>>,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 添加定时任务
    pub async fn add_job(&self, job: ScheduledJob) -> Result<(), CronError> {
        // 验证 schedule
        match &job.schedule {
            Schedule::Cron(cron_expr) => {
                // 验证 cron 表达式可以转换
                let _ = cron_expr.to_cron_string();
            }
            Schedule::NaturalLanguage(nl) => {
                if nl.is_empty() {
                    return Err(CronError::InvalidSchedule("Natural language description cannot be empty".into()));
                }
            }
            Schedule::Interval(dur) => {
                if dur.as_secs() == 0 {
                    return Err(CronError::InvalidSchedule("Interval cannot be zero".into()));
                }
            }
        }

        let job_id = job.id.clone();
        self.jobs.write().await.insert(job_id, job);
        Ok(())
    }

    /// 移除定时任务
    pub async fn remove_job(&self, job_id: &JobId) -> Result<ScheduledJob, JobError> {
        self.jobs
            .write()
            .await
            .remove(job_id)
            .ok_or_else(|| JobError::NotFound(job_id.clone()))
    }

    /// 获取任务
    pub async fn get_job(&self, job_id: &JobId) -> Option<ScheduledJob> {
        self.jobs.read().await.get(job_id).cloned()
    }

    /// 列出所有任务
    pub async fn list_jobs(&self) -> Vec<ScheduledJob> {
        self.jobs.read().await.values().cloned().collect()
    }

    /// 检查任务是否存在
    pub async fn has_job(&self, job_id: &JobId) -> bool {
        self.jobs.read().await.contains_key(job_id)
    }

    /// 启用/禁用任务
    pub async fn set_enabled(&self, job_id: &JobId, enabled: bool) -> Result<(), JobError> {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.enabled = enabled;
            Ok(())
        } else {
            Err(JobError::NotFound(job_id.clone()))
        }
    }

    /// 获取运行状态
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// 启动调度器 (预留接口)
    pub async fn start(&self) -> Result<(), CronError> {
        let mut running = self.running.write().await;
        if *running {
            return Err(CronError::Scheduler("Scheduler already running".into()));
        }
        *running = true;
        Ok(())
    }

    /// 停止调度器 (预留接口)
    pub async fn stop(&self) -> Result<(), CronError> {
        let mut running = self.running.write().await;
        if !*running {
            return Err(CronError::Scheduler("Scheduler not running".into()));
        }
        *running = false;
        Ok(())
    }

    /// 清空所有任务
    pub async fn clear(&self) {
        self.jobs.write().await.clear();
    }

    /// 获取任务数量
    pub async fn len(&self) -> usize {
        self.jobs.read().await.len()
    }

    /// 检查是否有任务
    pub async fn is_empty(&self) -> bool {
        self.jobs.read().await.is_empty()
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// 调度器扩展 Trait
#[async_trait]
pub trait JobScheduler: Send + Sync {
    /// 调度一个 Job
    async fn schedule(&self, job: ScheduledJob) -> Result<(), CronError>;

    /// 立即执行一个 Job
    async fn execute_now(&self, job_id: &JobId) -> Result<(), JobError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::CronExpression;

    #[tokio::test]
    async fn test_add_job() {
        let scheduler = CronScheduler::new();
        let job = ScheduledJob::new(
            JobId::new("test-job"),
            "Test Job".to_string(),
            Schedule::Cron(CronExpression::parse("0 9 * * *").unwrap()),
        );

        scheduler.add_job(job).await.unwrap();
        assert_eq!(scheduler.len().await, 1);
    }

    #[tokio::test]
    async fn test_remove_job() {
        let scheduler = CronScheduler::new();
        let job = ScheduledJob::new(
            JobId::new("test-job"),
            "Test Job".to_string(),
            Schedule::Cron(CronExpression::parse("0 9 * * *").unwrap()),
        );

        scheduler.add_job(job).await.unwrap();
        let removed = scheduler.remove_job(&JobId::new("test-job")).await.unwrap();
        assert_eq!(removed.name, "Test Job");
        assert!(scheduler.is_empty().await);
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let scheduler = CronScheduler::new();

        for i in 0..3 {
            let job = ScheduledJob::new(
                JobId::new(format!("job-{}", i)),
                format!("Job {}", i),
                Schedule::Cron(CronExpression::parse("0 9 * * *").unwrap()),
            );
            scheduler.add_job(job).await.unwrap();
        }

        let jobs = scheduler.list_jobs().await;
        assert_eq!(jobs.len(), 3);
    }

    #[tokio::test]
    async fn test_set_enabled() {
        let scheduler = CronScheduler::new();
        let job = ScheduledJob::new(
            JobId::new("test-job"),
            "Test Job".to_string(),
            Schedule::Cron(CronExpression::parse("0 9 * * *").unwrap()),
        );

        scheduler.add_job(job).await.unwrap();
        scheduler.set_enabled(&JobId::new("test-job"), false).await.unwrap();

        let job = scheduler.get_job(&JobId::new("test-job")).await.unwrap();
        assert!(!job.enabled);
    }

    #[tokio::test]
    async fn test_invalid_schedule() {
        let scheduler = CronScheduler::new();
        let job = ScheduledJob::new(
            JobId::new("test-job"),
            "Test Job".to_string(),
            Schedule::NaturalLanguage("".to_string()),
        );

        let result = scheduler.add_job(job).await;
        assert!(result.is_err());
    }
}
