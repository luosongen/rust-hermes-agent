//! McpServerBridge — MCP Server Bridge
//!
//! 实现 JSON-RPC 2.0 over stdio 协议，将本地工具通过 MCP 协议暴露。

use hermes_tool_registry::ToolRegistry;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::sync::Arc;

/// MCP JSON-RPC 请求
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    jsonrpc: String,
    #[serde(rename = "id")]
    id: serde_json::Value,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

/// MCP JSON-RPC 响应
#[derive(Debug, Serialize)]
pub struct McpResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<McpError>,
    #[serde(rename = "id")]
    id: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    code: i32,
    message: String,
}

/// MCP Server Bridge
pub struct McpServerBridge {
    registry: Arc<ToolRegistry>,
}

impl McpServerBridge {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    /// 启动 MCP 服务器主循环
    pub fn run(&self) -> Result<(), String> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut reader = io::BufReader::new(stdin).lines();

        loop {
            // 读取下一行 JSON
            let line = match reader.next() {
                Some(Ok(line)) => line,
                Some(Err(e)) => {
                    eprintln!("Error reading stdin: {}", e);
                    continue;
                }
                None => break, // EOF
            };

            if line.trim().is_empty() {
                continue;
            }

            // 解析请求
            let request: McpRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let error_resp = McpResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(McpError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                        }),
                        id: serde_json::Value::Null,
                    };
                    let _ = writeln!(stdout, "{}", serde_json::to_string(&error_resp).unwrap());
                    let _ = stdout.flush();
                    continue;
                }
            };

            // 处理请求
            let response = self.handle_request(request);

            // 发送响应
            let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
            let _ = stdout.flush();
        }

        Ok(())
    }

    /// 处理单个 JSON-RPC 请求
    fn handle_request(&self, request: McpRequest) -> McpResponse {
        let id = request.id.clone();

        match request.method.as_str() {
            "initialize" => McpResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "hermes-tools-extended",
                        "version": "0.1.0"
                    }
                })),
                error: None,
                id,
            },
            "tools/list" => {
                let tools = self.registry.get_tool_definitions();
                let tools_json: Vec<serde_json::Value> = tools
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.parameters
                        })
                    })
                    .collect();

                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    result: Some(json!({ "tools": tools_json })),
                    error: None,
                    id,
                }
            }
            "tools/call" => {
                let params = request.params.unwrap_or(serde_json::Value::Object(Default::default()));
                let tool_name = params["name"].as_str().unwrap_or("");

                match self.registry.get(tool_name) {
                    Some(_tool) => {
                        McpResponse {
                            jsonrpc: "2.0".to_string(),
                            result: Some(json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Tool '{}' registered. Execute via Agent.", tool_name)
                                }]
                            })),
                            error: None,
                            id,
                        }
                    }
                    None => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        result: None,
                        error: Some(McpError {
                            code: -32602,
                            message: format!("Tool not found: {}", tool_name),
                        }),
                        id,
                    },
                }
            }
            _ => McpResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
                id,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_tool_registry::ToolRegistry;
    use crate::{WebSearchTool, WebFetchTool};

    #[test]
    fn test_tools_list() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(WebSearchTool::new());
        registry.register(WebFetchTool::new());

        let bridge = McpServerBridge::new(registry);
        let tools = bridge.registry.get_tool_definitions();

        assert_eq!(tools.len(), 2);
        assert!(tools.iter().any(|t| t.name == "web_search"));
        assert!(tools.iter().any(|t| t.name == "web_fetch"));
    }

    #[test]
    fn test_mcp_response_serialization() {
        let response = McpResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(json!({ "tools": [] })),
            error: None,
            id: serde_json::Value::Number(1.into()),
        };

        let json_str = serde_json::to_string(&response).unwrap();
        assert!(json_str.contains("\"jsonrpc\":\"2.0\""));
        assert!(json_str.contains("\"result\""));
    }

    #[test]
    fn test_handle_request_initialize() {
        let registry = Arc::new(ToolRegistry::new());
        let bridge = McpServerBridge::new(registry);

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(1.into()),
            method: "initialize".to_string(),
            params: None,
        };

        let response = bridge.handle_request(request);
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_handle_request_tools_list() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register(WebSearchTool::new());
        let bridge = McpServerBridge::new(registry);

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(2.into()),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = bridge.handle_request(request);
        assert!(response.result.is_some());
        let result = response.result.unwrap();
        assert!(result.get("tools").is_some());
    }

    #[test]
    fn test_handle_request_method_not_found() {
        let registry = Arc::new(ToolRegistry::new());
        let bridge = McpServerBridge::new(registry);

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(3.into()),
            method: "unknown_method".to_string(),
            params: None,
        };

        let response = bridge.handle_request(request);
        assert!(response.error.is_some());
        assert!(response.error.unwrap().code == -32601);
    }
}
