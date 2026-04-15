//! terminal_tools — 内置终端执行工具
//!
//! 本模块提供 `TerminalTool`，供 AI Agent 在工作目录下执行 shell 命令。
//!
//! ## 主要类型
//! - **`TerminalTool`** — 终端执行工具（名称：`terminal`）
//!   - 参数：`command`（必填，执行的命令）、`timeout`（超时秒数，默认 30）
//!   - 行为：使用 `tokio::process::Command` 执行命令，返回 `stdout`、`stderr`、`exit_code`
//!   - 命令按空白字符拆分，不支持 shell 管道、重定向等复杂语法
//!   - 执行目录为 `context.working_directory`
//!
//! ## 返回格式
//! ```json
//! {
//!   "success": true,
//!   "command": "ls -la",
//!   "exit_code": 0,
//!   "stdout": "...",
//!   "stderr": "..."
//! }
//! ```
//!
//! ## 与其他模块的关系
//! - 实现 `hermes_tool_registry::Tool` trait
//! - 依赖 `hermes-core` 中的 `ToolContext`（获取工作目录）和 `ToolError`
//! - 内部使用 `tokio::process::Command` 进行异步进程管理

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
