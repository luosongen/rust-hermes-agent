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
use hermes_checkpoint::CheckpointManager;
use hermes_core::{ToolContext, ToolError};
use hermes_environment::{Environment, EnvironmentError};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// 终端命令执行工具
///
/// 在工作目录下执行 shell 命令并返回输出结果。
/// 支持通过不同的 `Environment` 后端执行（本地、Docker、SSH 等）。
///
/// # 工具参数
/// - `command`（必填）：要执行的命令
/// - `timeout`（可选）：超时时间（秒），默认 30
///
/// # 返回格式
/// JSON 包含 `success`、`command`、`exit_code`、`stdout`、`stderr`
///
/// # 安全说明
/// - 命令按空白字符拆分，不支持 shell 管道、重定向等复杂语法
/// - 自动检测危险命令并拒绝执行（无需审批工具介入）
/// - 相对路径基于 `context.working_directory` 解析
#[derive(Clone)]
pub struct TerminalTool {
    environment: Arc<dyn Environment>,
}

impl TerminalTool {
    /// 创建新的 TerminalTool 实例
    pub fn new(environment: Arc<dyn Environment>) -> Self {
        Self { environment }
    }

    /// 检查命令是否包含危险模式，返回 (是否危险, 原因)
    fn check_dangerous(command: &str) -> Option<String> {
        use regex::Regex;

        let patterns: Vec<(Regex, &str)> = vec![
            (Regex::new(r"\brm\s+(-[^\s]*\s*)*/").unwrap(), "delete in root path"),
            (Regex::new(r"\brm\s+-[^\s]*[rf]").unwrap(), "recursive or force delete"),
            (Regex::new(r"\brmdir\b").unwrap(), "remove directories"),
            (Regex::new(r"\bchmod\s+(-[^\s]*\s*)*(777|666|o\+[rwx]*w|a\+[rwx]*w)").unwrap(), "world-writable permissions"),
            (Regex::new(r"\bcurl\s+.*\|\s*bash").unwrap(), "pipe to bash (curl | bash)"),
            (Regex::new(r"\bwget\s+.*\|\s*bash").unwrap(), "pipe to bash (wget | bash)"),
            (Regex::new(r"\bsudo\s+su\b").unwrap(), "sudo su"),
            (Regex::new(r"\bsu\s+-\s*root").unwrap(), "switch to root"),
            (Regex::new(r"\btee\s+.*/etc/").unwrap(), "write to system directory"),
            (Regex::new(r"\bcat\s+.*>\s*/etc/").unwrap(), "redirect to system file"),
            (Regex::new(r"\biptables\s+(-[^\s]*\s*)*F").unwrap(), "flush iptables rules"),
            (Regex::new(r"\bufw\s+disable").unwrap(), "disable firewall"),
            (Regex::new(r"\bpkill\s+(-[^\s]*\s*)*-9").unwrap(), "force kill process"),
            (Regex::new(r"\bkillall\b").unwrap(), "kill all processes"),
            (Regex::new(r"\bmkfs\b").unwrap(), "format filesystem"),
            (Regex::new(r"\bdd\s+.*of=/dev/").unwrap(), "direct disk write"),
            (Regex::new(r"\bsystemctl\s+(stop|disable).*").unwrap(), "stop/disable service"),
            (Regex::new(r"\bmv\s+.*\s+/").unwrap(), "move to root"),
            (Regex::new(r"\bcp\s+.*\s+/").unwrap(), "copy to root"),
        ];

        for (re, reason) in patterns {
            if re.is_match(command) {
                return Some(reason.to_string());
            }
        }
        None
    }

    /// 检测命令是否有破坏性（会修改/删除文件）
    ///
    /// 破坏性命令执行前需要自动创建检查点。
    fn is_destructive(command: &str) -> bool {
        use regex::Regex;

        let destructive: Vec<Regex> = vec![
            Regex::new(r"\brm\b").unwrap(),
            Regex::new(r"\brmdir\b").unwrap(),
            Regex::new(r"\bmv\b").unwrap(),
            Regex::new(r"\bcp\b").unwrap(),
            Regex::new(r"\bdd\b").unwrap(),
            Regex::new(r"\btee\b").unwrap(),
            Regex::new(r"\{\s*>").unwrap(),       // 重定向输出（shell 语法）
            Regex::new(r"\d*>\s*[^>&]").unwrap(), // 输出重定向 > file
            Regex::new(r"\bgit\s+(reset|clean|checkout)\b").unwrap(),
            Regex::new(r"\bshred\b").unwrap(),
            Regex::new(r"\btruncate\b").unwrap(),
        ];

        for re in &destructive {
            if re.is_match(command) {
                return true;
            }
        }
        false
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

        // Security: auto-check dangerous commands before execution
        // YOLO 模式下跳过危险命令检查
        if !context.yolo_mode {
            if let Some(reason) = Self::check_dangerous(command) {
                return Err(ToolError::PermissionDenied(
                    format!("Dangerous command blocked: {}. Reason: {}. Use 'approval' tool if you need to whitelist this command.", command, reason)
                ));
            }
        }

        // 自动检查点：破坏性命令执行前对整个工作目录创建快照
        if Self::is_destructive(command) {
            if let Some(cm) = &context.checkpoint_manager {
                let _ = cm.snapshot_working_dir(&context.working_directory).await;
            }
        }

        let timeout_secs = args["timeout"].as_i64().unwrap_or(30);
        let timeout = Duration::from_secs(timeout_secs as u64);

        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(ToolError::InvalidArgs("Empty command".into()));
        }

        let (program, args_slice) = (parts[0], &parts[1..]);

        let result = self
            .environment
            .execute(program, args_slice, None, Some(timeout), None)
            .await
            .map_err(env_err_to_tool_err)?;

        Ok(json!({
            "success": result.success,
            "command": command,
            "exit_code": result.exit_code,
            "stdout": result.stdout,
            "stderr": result.stderr
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
