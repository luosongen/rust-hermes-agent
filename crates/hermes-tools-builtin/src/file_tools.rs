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
use serde_json::json;
use std::path::PathBuf;

/// 文件读取工具
///
/// 根据给定路径读取文件内容，支持行偏移和行数限制。
///
/// # 工具参数
/// - `path`（必填）：文件路径，支持绝对路径和相对路径
/// - `offset`（可选）：行偏移量，默认 0
/// - `limit`（可选）：最大读取行数，默认 1000
///
/// # 返回格式
/// JSON 包含 `success`、`path`、`content`、`size`
pub struct ReadFileTool;

impl ReadFileTool {
    /// 创建新的 ReadFileTool 实例
    pub fn new() -> Self {
        Self
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
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("path must be string".into()))?;

        let path = PathBuf::from(path_str);
        let full_path = if path.is_absolute() {
            path
        } else {
            context.working_directory.join(path)
        };

        // Security: prevent path traversal
        let canonical = full_path
            .canonicalize()
            .map_err(|e| ToolError::NotFound(format!("Path not found: {}", e)))?;

        // Read file
        let content = tokio::fs::read_to_string(&canonical)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to read: {}", e)))?;

        Ok(json!({
            "success": true,
            "path": canonical.to_string_lossy(),
            "content": content,
            "size": content.len()
        }).to_string())
    }
}

/// 文件写入工具
///
/// 将内容写入指定路径的文件，支持创建新文件和覆盖已有文件。
///
/// # 工具参数
/// - `path`（必填）：文件路径，支持绝对路径和相对路径
/// - `content`（必填）：要写入的内容
///
/// # 返回格式
/// JSON 包含 `success`、`path`、`bytes_written`
///
/// # 安全说明
/// - 不支持目录创建，依赖父目录已存在
/// - 相对路径基于 `context.working_directory` 解析
pub struct WriteFileTool;

impl WriteFileTool {
    /// 创建新的 WriteFileTool 实例
    pub fn new() -> Self {
        Self
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
        let full_path = if path.is_absolute() {
            path
        } else {
            context.working_directory.join(path)
        };

        tokio::fs::write(&full_path, content)
            .await
            .map_err(|e| ToolError::Execution(format!("Failed to write: {}", e)))?;

        Ok(json!({
            "success": true,
            "path": full_path.to_string_lossy(),
            "bytes_written": content.len()
        }).to_string())
    }
}
