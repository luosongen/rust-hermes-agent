//! DelegateTool — 子 Agent 并发执行工具
//!
//! 通过独立 hermes-cli 子进程执行子任务，支持受限工具集。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{Mutex, Semaphore};

const DEFAULT_MAX_CONCURRENT: usize = 3;

/// DelegateTool — 单例
pub struct DelegateTool {
    cli_path: PathBuf,
    config_dir: PathBuf,
    semaphore: Arc<Semaphore>,
    active_sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
}

struct SessionHandle {
    child: tokio::process::Child,
    stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateParams {
    pub goal: String,
    pub toolsets: Vec<String>,
    #[serde(default)]
    pub max_iterations: Option<u32>,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DelegateResult {
    pub status: String,
    pub summary: String,
    pub duration_secs: f64,
}

impl DelegateTool {
    pub fn new(cli_path: PathBuf, config_dir: PathBuf) -> Self {
        Self {
            cli_path,
            config_dir,
            semaphore: Arc::new(Semaphore::new(DEFAULT_MAX_CONCURRENT)),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Tool for DelegateTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a task to a subagent with isolated context and restricted toolsets."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "goal": { "type": "string", "description": "Task description for the subagent" },
                "toolsets": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Allowed toolsets for the subagent"
                },
                "max_iterations": { "type": "integer", "description": "Max agent iterations" },
                "context": { "type": "string", "description": "Additional context for the subagent" }
            },
            "required": ["goal", "toolsets"]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: DelegateParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // 等待并发槽位
        let _permit = self.semaphore.acquire().await
            .map_err(|e| ToolError::Execution(format!("Semaphore error: {}", e)))?;

        let task_id = context.session_id.clone();
        let start = std::time::Instant::now();

        // 构建子进程命令
        let mut cmd = Command::new(&self.cli_path);
        cmd.arg("agent");
        cmd.arg("--goal").arg(&params.goal);
        cmd.arg("--toolsets").arg(params.toolsets.join(","));
        cmd.arg("--session").arg(format!("delegate_{}", task_id));

        if let Some(max_iter) = params.max_iterations {
            cmd.arg("--max-iterations").arg(max_iter.to_string());
        }
        if let Some(ctx) = &params.context {
            cmd.arg("--context").arg(ctx);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()
            .map_err(|e| ToolError::Execution(format!("Failed to spawn delegate process: {}", e)))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| ToolError::Execution("No stdin for delegate process".to_string()))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| ToolError::Execution("No stdout for delegate process".to_string()))?;

        let stdin = Arc::new(Mutex::new(stdin));
        let mut reader = BufReader::new(stdout).lines();

        // 发送 start 消息
        {
            let mut s = stdin.lock().await;
            let msg = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "start",
                "params": {
                    "goal": &params.goal,
                    "toolsets": &params.toolsets,
                    "context": params.context
                }
            });
            s.write_all(format!("{}\n", msg).as_bytes()).await
                .map_err(|e| ToolError::Execution(format!("Failed to send start: {}", e)))?;
        }

        // 读取响应（直到收到 result 或 error）
        let mut result_str = String::new();
        while let Some(line) = reader.next_line().await
            .map_err(|e| ToolError::Execution(format!("Read error: {}", e)))? {
            if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&line) {
                if resp.get("result").is_some() || resp.get("error").is_some() {
                    result_str = line;
                    break;
                }
            }
        }

        // 终止子进程
        child.kill().await.ok();
        let _ = child.wait().await;

        let duration = start.elapsed().as_secs_f64();

        if result_str.is_empty() {
            return Err(ToolError::Execution("No result from delegate process".to_string()));
        }

        let resp: serde_json::Value = serde_json::from_str(&result_str)
            .map_err(|e| ToolError::Execution(format!("Invalid result JSON: {}", e)))?;

        if let Some(error) = resp.get("error") {
            return Err(ToolError::Execution(
                error.as_str().unwrap_or("Delegate failed").to_string()
            ));
        }

        let result = resp["result"].clone();

        Ok(json!({
            "status": result["status"].as_str().unwrap_or("success"),
            "summary": result["summary"].as_str().unwrap_or(""),
            "duration_secs": duration,
            "provider": result["provider"].as_str().unwrap_or("unknown")
        }).to_string())
    }
}
