//! CronScheduler — 定时任务调度工具
//!
//! 允许安排工具在指定时间执行。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde_json::json;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use parking_lot::RwLock;

/// 定时任务结构
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScheduledJob {
    pub id: String,
    pub cron_expression: String,
    pub tool_name: String,
    pub tool_args: serde_json::Value,
}

/// CronScheduler — 定时任务调度工具
pub struct CronScheduler {
    jobs: Arc<RwLock<HashMap<String, ScheduledJob>>>,
    counter: Arc<RwLock<u64>>,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            counter: Arc::new(RwLock::new(0)),
        }
    }

    fn generate_id(&self) -> String {
        let mut counter = self.counter.write();
        *counter += 1;
        format!("job_{}", *counter)
    }

    pub fn schedule(
        &self,
        cron_expression: &str,
        tool_name: &str,
        tool_args: serde_json::Value,
    ) -> Result<String, String> {
        // Normalize to 6-field cron (with seconds) if 5-field provided
        let normalized = if cron_expression.split_whitespace().count() == 5 {
            format!("0 {}", cron_expression)
        } else {
            cron_expression.to_string()
        };

        // 验证 cron 表达式
        cron::Schedule::from_str(&normalized)
            .map_err(|e| format!("Invalid cron expression: {}", e))?;

        let id = self.generate_id();
        let job = ScheduledJob {
            id: id.clone(),
            cron_expression: cron_expression.to_string(),
            tool_name: tool_name.to_string(),
            tool_args,
        };

        self.jobs.write().insert(id.clone(), job);
        Ok(id)
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), String> {
        let mut jobs = self.jobs.write();
        if jobs.remove(job_id).is_some() {
            Ok(())
        } else {
            Err(format!("Job not found: {}", job_id))
        }
    }

    pub fn list(&self) -> Vec<ScheduledJob> {
        self.jobs.read().values().cloned().collect()
    }
}

impl Default for CronScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CronScheduler {
    fn name(&self) -> &str {
        "cron_schedule"
    }

    fn description(&self) -> &str {
        "Schedule a tool to run at a specified cron time. Use action 'list' to view scheduled jobs, 'cancel' to remove a job."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["schedule", "cancel", "list"],
                    "description": "The action to perform"
                },
                "cron_expression": {
                    "type": "string",
                    "description": "Cron expression (min hour day month weekday), e.g., '0 9 * * *' for 9 AM daily"
                },
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool to schedule"
                },
                "tool_args": {
                    "type": "object",
                    "description": "Arguments to pass to the tool"
                },
                "job_id": {
                    "type": "string",
                    "description": "Job ID to cancel"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("action is required".to_string()))?;

        match action {
            "schedule" => {
                let cron_expr = args["cron_expression"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("cron_expression is required".to_string()))?;

                let tool_name = args["tool_name"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("tool_name is required".to_string()))?;

                let tool_args = args["tool_args"].clone();

                match self.schedule(cron_expr, tool_name, tool_args) {
                    Ok(job_id) => Ok(json!({ "scheduled": true, "job_id": job_id }).to_string()),
                    Err(e) => Err(ToolError::Execution(e)),
                }
            }
            "cancel" => {
                let job_id = args["job_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidArgs("job_id is required".to_string()))?;

                match self.cancel(job_id) {
                    Ok(()) => Ok(json!({ "cancelled": true, "job_id": job_id }).to_string()),
                    Err(e) => Err(ToolError::Execution(e)),
                }
            }
            "list" => {
                let jobs = self.list();
                Ok(serde_json::to_string_pretty(&jobs).unwrap_or_else(|_| "[]".to_string()))
            }
            _ => Err(ToolError::InvalidArgs(format!(
                "Unknown action: {}. Use: schedule, cancel, list",
                action
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_job() {
        let scheduler = CronScheduler::new();
        let result = scheduler.schedule("0 9 * * *", "web_search", json!({"query": "test"}));
        assert!(result.is_ok());
        let job_id = result.unwrap();
        assert!(job_id.starts_with("job_"));
    }

    #[test]
    fn test_list_jobs() {
        let scheduler = CronScheduler::new();
        scheduler.schedule("0 9 * * *", "web_search", json!({})).unwrap();
        scheduler.schedule("0 10 * * *", "web_fetch", json!({"url": "test"})).unwrap();

        let jobs = scheduler.list();
        assert_eq!(jobs.len(), 2);
    }

    #[test]
    fn test_cancel_job() {
        let scheduler = CronScheduler::new();
        let job_id = scheduler.schedule("0 9 * * *", "web_search", json!({})).unwrap();

        let result = scheduler.cancel(&job_id);
        assert!(result.is_ok());

        let jobs = scheduler.list();
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_invalid_cron() {
        let scheduler = CronScheduler::new();
        let result = scheduler.schedule("invalid", "web_search", json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid cron"));
    }

    #[test]
    fn test_name_and_description() {
        let scheduler = CronScheduler::new();
        assert_eq!(scheduler.name(), "cron_schedule");
        assert!(!scheduler.description().is_empty());
    }
}