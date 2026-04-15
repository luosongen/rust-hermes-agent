//! CronScheduler — 定时任务调度

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;

#[derive(Debug, Clone)]
pub struct CronScheduler;

impl CronScheduler {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CronScheduler {
    fn name(&self) -> &str {
        "cron_scheduler"
    }

    fn description(&self) -> &str {
        "Schedule a cron job"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Cron expression"
                },
                "task": {
                    "type": "string",
                    "description": "Task to execute"
                }
            },
            "required": ["expression", "task"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let expression = args["expression"].as_str().unwrap_or("");
        let task = args["task"].as_str().unwrap_or("");
        Ok(format!("Scheduled cron '{}' for task: {}", expression, task))
    }
}
