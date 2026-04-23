//! 环境错误类型

use thiserror::Error;

/// 环境执行错误
#[derive(Error, Debug)]
pub enum EnvironmentError {
    #[error("Execution failed: {0}")]
    Execution(String),

    #[error("Command not found: {0}")]
    CommandNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Authentication failed: {0}")]
    Authentication(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Not supported in {backend}: {operation}")]
    NotSupported { backend: String, operation: String },
}

/// 命令执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 命令字符串（用于日志和调试）
    pub command: String,
    /// 退出码（None 表示被信号终止）
    pub exit_code: Option<i32>,
    /// 标准输出
    pub stdout: String,
    /// 标准错误
    pub stderr: String,
    /// 是否成功（exit_code == 0）
    pub success: bool,
}

impl ExecutionResult {
    /// 创建一个新的执行结果
    pub fn new(command: impl Into<String>, exit_code: Option<i32>, stdout: String, stderr: String) -> Self {
        let success = exit_code == Some(0);
        Self {
            command: command.into(),
            exit_code,
            stdout,
            stderr,
            success,
        }
    }
}
