//! MCP Protocol Types

use serde::{Deserialize, Serialize};

/// MCP 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<usize>,
}

impl McpRequest {
    pub fn new(method: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: None,
            id: None,
        }
    }

    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.params = Some(params);
        self
    }

    pub fn with_id(mut self, id: usize) -> Self {
        self.id = Some(id);
        self
    }
}

/// MCP 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<McpError>,
    pub id: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// MCP 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    pub fn new(name: &str, description: &str, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
        }
    }
}

/// MCP 工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// MCP 初始化参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// MCP 工具列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<ToolDefinition>,
}

/// MCP 工具调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContent {
    pub r#type: String,
    pub text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_new() {
        let req = McpRequest::new("tools/list");
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "tools/list");
    }

    #[test]
    fn test_request_with_params() {
        let req = McpRequest::new("tools/call")
            .with_params(serde_json::json!({"name": "test"}))
            .with_id(1);
        assert!(req.params.is_some());
        assert_eq!(req.id, Some(1));
    }

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition::new(
            "test_tool",
            "A test tool",
            serde_json::json!({"type": "object"}),
        );
        assert_eq!(tool.name, "test_tool");
    }
}