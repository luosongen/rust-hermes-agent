//! CodeExecutionTool — PTC (Programmatic Tool Calling)
//!
//! 让 LLM 写 Python 脚本，通过 RPC 调用 hermes 工具。
//! 支持通过 `Environment` 后端执行代码，实现远程代码执行。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_environment::{Environment, EnvironmentError};
use hermes_tool_registry::Tool;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Duration;

/// CodeExecutionTool — 单例
#[derive(Clone)]
pub struct CodeExecutionTool {
    store: Arc<RwLock<ExecutionStore>>,
    config: ExecutionConfig,
    environment: Arc<dyn Environment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    pub allowed_tools: Vec<String>,
    pub timeout_secs: u64,
    pub max_tool_calls: u32,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            allowed_tools: vec![
                "web_search".to_string(),
                "web_extract".to_string(),
                "read_file".to_string(),
                "write_file".to_string(),
                "search_files".to_string(),
                "patch".to_string(),
                "terminal".to_string(),
            ],
            timeout_secs: 300,
            max_tool_calls: 50,
            max_stdout_bytes: 50_000,
            max_stderr_bytes: 10_000,
        }
    }
}

pub struct ExecutionStore {
    pending: HashMap<String, ExecutionHandle>,
}

pub struct ExecutionHandle {
    pub task_id: String,
    pub status: String,
    pub start_time: f64,
}

impl Default for ExecutionStore {
    fn default() -> Self {
        Self { pending: HashMap::new() }
    }
}

impl CodeExecutionTool {
    pub fn new(config: ExecutionConfig, environment: Arc<dyn Environment>) -> Self {
        Self {
            store: Arc::new(RwLock::new(ExecutionStore::default())),
            config,
            environment,
        }
    }

    /// Generate hermes_tools.py stub for subprocess
    pub fn generate_stub(&self, mode: &str, socket_path: Option<&str>) -> String {
        let tools = self.config.allowed_tools.join(", ");
        match mode {
            "uds" => {
                let socket = socket_path.unwrap_or("/tmp/hermes_uds.sock");
                format!(r#"import json
import socket
import sys

SOCKET_PATH = "{socket}"

def _rpc(method, params):
    msg = json.dumps({{"jsonrpc": "2.0", "method": method, "params": params, "id": 1}})
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect(SOCKET_PATH)
    sock.sendall((msg + "\n").encode())
    resp = sock.recv(4096).decode()
    sock.close()
    return json.loads(resp)

def read_file(path):
    return _rpc("read_file", {{"path": path}})

def write_file(path, content):
    return _rpc("write_file", {{"path": path, "content": content}})

def terminal(cmd):
    return _rpc("terminal", {{"command": cmd}})

def web_search(query):
    return _rpc("web_search", {{"query": query}})

ALLOWED_TOOLS = [{tools}]
"#)
            }
            _ => {
                // File RPC mode stub
                format!(r#"import json
import os
import time
import tempfile

ALLOWED_TOOLS = [{tools}]
REQ_DIR = tempfile.mkdtemp(prefix="hermes_req_")

def _rpc(method, params):
    req_id = os.urandom(8).hex()
    req_file = os.path.join(REQ_DIR, f"{{req_id}}.req")
    resp_file = os.path.join(REQ_DIR, f"{{req_id}}.resp")
    with open(req_file, "w") as f:
        json.dump({{"method": method, "params": params, "id": req_id}}, f)
    while not os.path.exists(resp_file):
        time.sleep(0.1)
    with open(resp_file, "r") as f:
        return json.load(f)

def read_file(path):
    return _rpc("read_file", {{"path": path}})

def write_file(path, content):
    return _rpc("write_file", {{"path": path, "content": content}})

def terminal(cmd):
    return _rpc("terminal", {{"command": cmd}})

def web_search(query):
    return _rpc("web_search", {{"query": query}})
"#)
            }
        }
    }
}

#[async_trait]
impl Tool for CodeExecutionTool {
    fn name(&self) -> &str { "execute_code" }

    fn description(&self) -> &str {
        "Execute Python code that calls Hermes tools via RPC. Supports local (UDS) and remote (file-based) modes."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute" },
                "language": { "type": "string", "enum": ["python"], "default": "python" },
                "timeout_secs": { "type": "integer" }
            },
            "required": ["code"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let code = args["code"].as_str()
            .ok_or_else(|| ToolError::InvalidArgs("code is required".to_string()))?;
        let timeout_secs = args["timeout_secs"]
            .as_u64()
            .unwrap_or(self.config.timeout_secs);

        // 检测是否跨平台（代码中包含 SSH/Docker 等则为远程）
        let is_remote = code.contains("SSH") || code.contains("docker") || code.contains("modal");
        let mode = if is_remote { "file" } else { "uds" };
        let stub = self.generate_stub(mode, None);

        // Write stub and code to temp files
        let tmpdir = tempfile::tempdir()
            .map_err(|e| ToolError::Execution(format!("Temp dir error: {}", e)))?;
        let stub_path = tmpdir.path().join("hermes_tools.py");
        let code_path = tmpdir.path().join("user_script.py");

        std::fs::write(&stub_path, &stub)
            .map_err(|e| ToolError::Execution(format!("Stub write error: {}", e)))?;
        std::fs::write(&code_path, code)
            .map_err(|e| ToolError::Execution(format!("Code write error: {}", e)))?;

        // 使用 Environment 执行 Python 进程
        let result = self
            .environment
            .execute(
                "python3",
                &[code_path.to_string_lossy().as_ref()],
                Some(tmpdir.path()),
                Some(Duration::from_secs(timeout_secs)),
                Some(&[("PYTHONPATH".to_string(), tmpdir.path().to_string_lossy().to_string())].into_iter().collect()),
            )
            .await
            .map_err(env_err_to_tool_err)?;

        let truncated_stdout = if result.stdout.len() > self.config.max_stdout_bytes {
            format!("{}...[truncated {} bytes]", &result.stdout[..self.config.max_stdout_bytes], result.stdout.len() - self.config.max_stdout_bytes)
        } else {
            result.stdout
        };

        let truncated_stderr = if result.stderr.len() > self.config.max_stderr_bytes {
            format!("{}...[truncated {} bytes]", &result.stderr[..self.config.max_stderr_bytes], result.stderr.len() - self.config.max_stderr_bytes)
        } else {
            result.stderr
        };

        Ok(json!({
            "success": result.success,
            "stdout": truncated_stdout,
            "stderr": truncated_stderr,
            "exit_code": result.exit_code,
            "mode": mode
        }).to_string())
    }
}

/// 将 EnvironmentError 转换为 ToolError
fn env_err_to_tool_err(e: EnvironmentError) -> ToolError {
    match e {
        EnvironmentError::Execution(msg) => ToolError::Execution(msg),
        EnvironmentError::CommandNotFound(cmd) => ToolError::Execution(format!("Command not found: {}", cmd)),
        EnvironmentError::PermissionDenied(msg) => ToolError::PermissionDenied(msg),
        EnvironmentError::PathNotFound(msg) => ToolError::NotFound(msg),
        EnvironmentError::Connection(msg) => ToolError::Execution(format!("Connection failed: {}", msg)),
        EnvironmentError::Authentication(msg) => ToolError::Execution(format!("Authentication failed: {}", msg)),
        EnvironmentError::Timeout(msg) => ToolError::Timeout(msg),
        EnvironmentError::InvalidConfig(msg) => ToolError::Execution(format!("Invalid config: {}", msg)),
        EnvironmentError::Io(err) => ToolError::Execution(format!("IO error: {}", err)),
        EnvironmentError::NotSupported { backend, operation } => {
            ToolError::Execution(format!("{} not supported in {}", operation, backend))
        }
    }
}
