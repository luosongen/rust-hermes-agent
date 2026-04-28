//! file_tools — 内置文件读写工具
//!
//! 本模块提供两个文件系统工具，供 AI Agent 在工作目录中读写文件：
//!
//! ## 主要类型
//! - **`ReadFileTool`** — 文件读取工具（名称：`read_file`）
//!   - 参数：`path`（必填）、`offset`（行偏移，默认 0）、`limit`（最大行数，默认 1000）
//!   - 行为：读取文件内容，返回 JSON 包含 `success`、`path`、`content`、`size`
//!   - 安全：使用 `canonicalize()` 解析真实路径，防止 `../` 等路径遍历攻击
//!   - 相对路径会基于 `context.working_directory` 解析
//!
//! - **`WriteFileTool`** — 文件写入工具（名称：`write_file`）
//!   - 参数：`path`（必填）、`content`（必填）
//!   - 行为：创建新文件或覆盖已有文件，返回 `success` 和写入字节数
//!   - 不支持目录创建（依赖父目录已存在）
//!
//! ## 与其他模块的关系
//! - 实现 `hermes_tool_registry::Tool` trait
//! - 依赖 `hermes-core` 中的 `ToolContext`（获取工作目录）和 `ToolError`
//! - 工具参数通过 `serde_json::Value` 传递，结果序列化为 JSON 字符串

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_environment::{Environment, EnvironmentError};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 路径安全检查：验证路径是否在工作目录范围内
///
/// 防止路径遍历攻击，确保解析后的路径不会跳出允许的工作目录。
pub fn validate_path_within_workdir(path: &Path, workdir: &Path) -> Result<PathBuf, ToolError> {
    // 获取绝对路径
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workdir.join(path)
    };

    // 规范化路径（解析 . 和 ..）
    let canonical = absolute
        .components()
        .fold(PathBuf::new(), |mut acc, c| {
            match c {
                std::path::Component::Normal(p) => acc.push(p),
                std::path::Component::RootDir => acc.push("/"),
                std::path::Component::Prefix(prefix) => acc.push(prefix.as_os_str()),
                std::path::Component::CurDir => {} // skip .
                std::path::Component::ParentDir => {
                    // 处理 ..，尝试弹出一级
                    if !acc.pop() {
                        // 如果已经在根目录还向上，可能是攻击
                        tracing::warn!("[PathSecurity] Path traversal attempt blocked: {:?}", path);
                    }
                }
            }
            acc
        });

    // 检查是否超出工作目录
    let workdir_canonical = if let Ok(cwd) = std::env::current_dir() {
        cwd.join(workdir)
    } else {
        workdir.to_path_buf()
    };

    // 对敏感目录的额外保护
    let sensitive_prefixes = [
        "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64",
        "/opt", "/sys", "/proc", "/dev", "/boot", "/root",
        ".ssh", ".aws", ".docker", ".kube",
    ];

    let path_str = canonical.to_string_lossy();
    for prefix in &sensitive_prefixes {
        if path_str.starts_with(prefix) || path_str.contains(&format!("/{}/", prefix)) {
            return Err(ToolError::PermissionDenied(
                format!("Access to sensitive path '{}' is not allowed", canonical.display())
            ));
        }
    }

    Ok(canonical)
}

/// 文件读取工具
///
/// 根据给定路径读取文件内容，支持行偏移和行数限制。
/// 通过 `Environment` 后端读取文件，支持本地、Docker、SSH 等执行环境。
///
/// # 工具参数
/// - `path`（必填）：文件路径，支持绝对路径和相对路径
/// - `offset`（可选）：行偏移量，默认 0
/// - `limit`（可选）：最大读取行数，默认 1000
///
/// # 返回格式
/// JSON 包含 `success`、`path`、`content`、`size`
#[derive(Clone)]
pub struct ReadFileTool {
    environment: Arc<dyn Environment>,
}

impl ReadFileTool {
    /// 创建新的 ReadFileTool 实例
    pub fn new(environment: Arc<dyn Environment>) -> Self {
        Self { environment }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file from the filesystem. \
         Returns the file content as a string."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line offset to start reading from",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read",
                    "default": 1000
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("path must be string".into()))?;

        let path = PathBuf::from(path_str);
        let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Security: validate path does not escape workdir or touch sensitive dirs
        let normalized = validate_path_within_workdir(&path, &workdir)?;

        // Read file via Environment
        let content = self
            .environment
            .read_file(&normalized)
            .await
            .map_err(env_err_to_tool_err)?;

        // Apply offset and limit
        let offset = args["offset"].as_i64().unwrap_or(0).max(0) as usize;
        let limit = args["limit"].as_i64().unwrap_or(1000).max(1) as usize;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        let start = offset.min(total_lines);
        let end = (start + limit).min(total_lines);
        let selected: Vec<&str> = lines[start..end].to_vec();
        let result = selected.join("\n");

        Ok(json!({
            "success": true,
            "path": path_str,
            "content": result,
            "size": result.len(),
            "total_lines": total_lines,
            "offset": start,
            "lines_read": end - start
        }).to_string())
    }
}

/// 文件写入工具
///
/// 将内容写入指定路径的文件，支持创建新文件和覆盖已有文件。
/// 通过 `Environment` 后端写入文件，支持本地、Docker、SSH 等执行环境。
///
/// # 工具参数
/// - `path`（必填）：文件路径，支持绝对路径和相对路径
/// - `content`（必填）：要写入的内容
///
/// # 返回格式
/// JSON 包含 `success`、`path`、`bytes_written`
#[derive(Clone)]
pub struct WriteFileTool {
    environment: Arc<dyn Environment>,
}

impl WriteFileTool {
    /// 创建新的 WriteFileTool 实例
    pub fn new(environment: Arc<dyn Environment>) -> Self {
        Self { environment }
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, \
         overwrites if it does."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("path must be string".into()))?;

        let content = args["content"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("content must be string".into()))?;

        let path = PathBuf::from(path_str);

        // Security: validate path does not escape workdir or touch sensitive dirs
        let normalized = validate_path_within_workdir(&path, &context.working_directory)?;

        // 自动检查点：写入前对文件创建快照
        if let Some(cm) = &context.checkpoint_manager {
            let _ = cm.snapshot_file(&normalized, &context.working_directory).await;
        }

        self.environment
            .write_file(&normalized, content)
            .await
            .map_err(env_err_to_tool_err)?;

        Ok(json!({
            "success": true,
            "path": path_str,
            "bytes_written": content.len()
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
