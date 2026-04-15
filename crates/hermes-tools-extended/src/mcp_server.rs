//! McpServerBridge — MCP Server Bridge

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;

#[derive(Debug, Clone)]
pub struct McpServerBridge;

impl McpServerBridge {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for McpServerBridge {
    fn name(&self) -> &str {
        "mcp_server"
    }

    fn description(&self) -> &str {
        "MCP Server Bridge"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "MCP command"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let command = args["command"].as_str().unwrap_or("");
        Ok(format!("MCP result for: {}", command))
    }
}
