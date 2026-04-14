use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use serde_json::json;
use std::path::PathBuf;

pub struct ReadFileTool;

impl ReadFileTool {
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

pub struct WriteFileTool;

impl WriteFileTool {
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
