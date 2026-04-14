use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use serde_json::json;
use std::process::Stdio;

pub struct TerminalTool;

impl TerminalTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for TerminalTool {
    fn name(&self) -> &str {
        "terminal"
    }

    fn description(&self) -> &str {
        "Execute a terminal command and return the output."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in seconds",
                    "default": 30
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("command must be string".into()))?;

        let _timeout_secs = args["timeout"].as_i64().unwrap_or(30);

        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(ToolError::InvalidArgs("Empty command".into()));
        }

        let (program, args_slice) = (parts[0], &parts[1..]);

        let output = tokio::process::Command::new(program)
            .args(args_slice)
            .current_dir(&context.working_directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to execute: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(json!({
            "success": true,
            "command": command,
            "exit_code": output.status.code(),
            "stdout": stdout,
            "stderr": stderr
        }).to_string())
    }
}
