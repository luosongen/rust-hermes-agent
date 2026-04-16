//! McpClientBridge — MCP Client Bridge
//!
//! Connects as a client to external MCP servers over stdio, exposing remote tools
//! as locally-namespaced tools.

use async_trait::async_trait;
use hermes_core::{ToolCall, ToolContext, ToolDefinition, ToolDispatcher, ToolError};
use hermes_tool_registry::Tool;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex as AsyncMutex;

/// MCP JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: i64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    fn new(id: i64, method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// MCP JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    id: serde_json::Value,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<McpErrorResponse>,
}

#[derive(Debug, Deserialize)]
struct McpErrorResponse {
    code: i32,
    message: String,
}

/// Initialize request params
#[derive(Debug, Serialize)]
struct InitializeParams {
    protocol_version: String,
    capabilities: serde_json::Value,
    client_info: ClientInfo,
}

#[derive(Debug, Serialize)]
struct ClientInfo {
    name: String,
    version: String,
}

/// Tools/list response
#[derive(Debug, Deserialize)]
struct ToolsListResponse {
    tools: Vec<RemoteTool>,
}

#[derive(Debug, Deserialize)]
struct RemoteTool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

/// tools/call response
#[derive(Debug, Deserialize)]
struct ToolsCallResponse {
    content: Vec<ContentItem>,
}

#[derive(Debug, Deserialize)]
struct ContentItem {
    #[serde(rename = "type")]
    item_type: String,
    text: Option<String>,
}

/// Wrapper for a remote tool exposed by McpClientBridge
pub struct McpTool {
    full_name: String,
    description: String,
    input_schema: serde_json::Value,
    client: Arc<McpClientBridge>,
}

impl McpTool {
    fn new(server_name: &str, remote_name: &str, description: String, input_schema: serde_json::Value, client: Arc<McpClientBridge>) -> Self {
        Self {
            full_name: format!("{}.{}", server_name, remote_name),
            description,
            input_schema,
            client,
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> serde_json::Value {
        self.input_schema.clone()
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        self.client.call_tool(&self.full_name, args).await
    }
}

/// MCP Client Bridge — connects to external MCP servers over stdio
pub struct McpClientBridge {
    server_name: String,
    command: String,
    args: Vec<String>,
    child: Option<tokio::process::Child>,
    writer: Arc<AsyncMutex<tokio::io::BufWriter<ChildStdin>>>,
    reader: Arc<AsyncMutex<BufReader<ChildStdout>>>,
    request_id: Arc<Mutex<i64>>,
    tools: Arc<Mutex<Vec<ToolDefinition>>>,
}

impl McpClientBridge {
    /// Connect to an MCP server
    pub async fn connect(
        server_name: &str,
        command: &str,
        args: &[String],
    ) -> Result<Self, ToolError> {
        // Spawn the child process
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ToolError::Execution(format!("Failed to spawn MCP server: {}", e)))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            ToolError::Execution("Failed to take stdin from child process".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            ToolError::Execution("Failed to take stdout from child process".to_string())
        })?;

        let writer = Arc::new(AsyncMutex::new(tokio::io::BufWriter::new(stdin)));
        let reader = Arc::new(AsyncMutex::new(BufReader::new(stdout)));
        let request_id = Arc::new(Mutex::new(0));
        let tools = Arc::new(Mutex::new(Vec::new()));

        let mut client = Self {
            server_name: server_name.to_string(),
            command: command.to_string(),
            args: args.to_vec(),
            child: Some(child),
            writer,
            reader,
            request_id,
            tools,
        };

        // Send initialize request
        client.send_initialize().await?;

        // Send tools/list request
        let remote_tools = client.send_tools_list().await?;

        // Register tools with namespaced names
        let mut registered_tools = Vec::new();
        for tool in remote_tools {
            let full_name = format!("{}.{}", server_name, tool.name);
            registered_tools.push(ToolDefinition {
                name: full_name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            });
        }

        *client.tools.lock() = registered_tools;

        Ok(client)
    }

    /// Send initialize request
    async fn send_initialize(&mut self) -> Result<(), ToolError> {
        let id = self.next_id();

        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({}),
            client_info: ClientInfo {
                name: "hermes-agent".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let request = JsonRpcRequest::new(id, "initialize", Some(serde_json::to_value(params).unwrap()));
        self.send_request(request).await?;

        // Wait for response
        let response: JsonRpcResponse = self.read_response(id).await?;
        if response.error.is_some() {
            let err = response.error.unwrap();
            return Err(ToolError::Execution(format!("Initialize failed: {} (code {})", err.message, err.code)));
        }

        // Send notifications/initialized
        let notif = JsonRpcRequest::new(self.next_id(), "notifications/initialized", None);
        self.send_request(notif).await?;

        Ok(())
    }

    /// Send tools/list request
    async fn send_tools_list(&mut self) -> Result<Vec<RemoteTool>, ToolError> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "tools/list", None);
        self.send_request(request).await?;

        let response: JsonRpcResponse = self.read_response(id).await?;

        if let Some(err) = response.error {
            return Err(match err.code {
                -32601 => ToolError::InvalidArgs(format!("Method not found: {}", err.message)),
                -32602 => ToolError::InvalidArgs(format!("Invalid parameters: {}", err.message)),
                _ => ToolError::Execution(format!("RPC error: {} (code {})", err.message, err.code)),
            });
        }

        let result = response.result.ok_or_else(|| {
            ToolError::Execution("No result in tools/list response".to_string())
        })?;

        let tools_resp: ToolsListResponse = serde_json::from_value(result).map_err(|e| {
            ToolError::Execution(format!("Failed to parse tools/list response: {}", e))
        })?;

        Ok(tools_resp.tools)
    }

    /// Send a JSON-RPC request
    async fn send_request(&self, request: JsonRpcRequest) -> Result<(), ToolError> {
        let json = serde_json::to_string(&request).map_err(|e| {
            ToolError::Execution(format!("Failed to serialize request: {}", e))
        })?;

        let line = format!("{}\n", json);

        // Use get_mut to get mutable reference and release lock immediately
        {
            let mut writer = self.writer.lock().await;
            tokio::io::AsyncWriteExt::write_all(&mut *writer, line.as_bytes()).await.map_err(|e| {
                ToolError::Execution(format!("Failed to write to stdin: {}", e))
            })?;
            tokio::io::AsyncWriteExt::flush(&mut *writer).await.map_err(|e| {
                ToolError::Execution(format!("Failed to flush stdin: {}", e))
            })?;
        }

        Ok(())
    }

    /// Read responses until we find the one matching the given id
    async fn read_response(&self, target_id: i64) -> Result<JsonRpcResponse, ToolError> {
        let mut line = String::new();

        // Read lines until we find matching id or get an error
        loop {
            line.clear();
            // Use get_mut to get mutable reference and release lock immediately
            let read_result = {
                let mut reader = self.reader.lock().await;
                tokio::io::AsyncBufReadExt::read_line(&mut *reader, &mut line).await
            };

            match read_result {
                Ok(0) => return Err(ToolError::Execution("EOF reading from MCP server".to_string())),
                Ok(_) => {}
                Err(e) => return Err(ToolError::Execution(format!("Failed to read from stdout: {}", e))),
            }

            if line.trim().is_empty() {
                continue;
            }

            let response: JsonRpcResponse = match serde_json::from_str(&line) {
                Ok(resp) => resp,
                Err(_) => {
                    // Parse error - could be a notification or invalid JSON
                    // Try to parse error code to determine retry
                    if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&line) {
                        if let Some(err) = resp.error {
                            if err.code == -32700 {
                                // Parse error - log and retry
                                eprintln!("MCP parse error, retrying: {}", err.message);
                                continue;
                            }
                        }
                    }
                    continue;
                }
            };

            // Check if this is the response we want
            if let Some(id_val) = response.id.as_i64() {
                if id_val == target_id {
                    return Ok(response);
                }
            } else if let Some(id_val) = response.id.as_str() {
                if let Ok(parsed) = id_val.parse::<i64>() {
                    if parsed == target_id {
                        return Ok(response);
                    }
                }
            } else if response.id.is_null() {
                // Notification or error response without id - skip
                continue;
            }

            // Out-of-order response - continue reading
            continue;
        }
    }

    /// Get the next request ID
    fn next_id(&self) -> i64 {
        let mut id = self.request_id.lock();
        *id += 1;
        *id
    }

    /// Disconnect from the MCP server
    pub fn disconnect(&mut self) -> Result<(), ToolError> {
        if let Some(ref mut child) = self.child {
            child.start_kill().map_err(|e| {
                ToolError::Execution(format!("Failed to kill MCP server process: {}", e))
            })?;
        }
        self.child = None;
        Ok(())
    }

    /// List tools available from the connected server
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools.lock().clone()
    }

    /// Call a remote tool
    pub async fn call_tool(&self, tool_name: &str, arguments: serde_json::Value) -> Result<String, ToolError> {
        let id = {
            let mut counter = self.request_id.lock();
            *counter += 1;
            *counter
        };

        let params = json!({
            "name": tool_name,
            "arguments": arguments
        });

        let request = JsonRpcRequest::new(id, "tools/call", Some(params));
        self.send_request(request).await?;

        let response: JsonRpcResponse = self.read_response(id).await?;

        if let Some(err) = response.error {
            return Err(match err.code {
                -32601 => ToolError::InvalidArgs(format!("Method not found: {}", err.message)),
                -32602 => ToolError::InvalidArgs(format!("Invalid parameters: {}", err.message)),
                -32000 => ToolError::Execution(format!("Connection error: {}", err.message)),
                -32001 => ToolError::Timeout(format!("Request timeout: {}", err.message)),
                _ => ToolError::Execution(format!("RPC error: {} (code {})", err.message, err.code)),
            });
        }

        let result = response.result.ok_or_else(|| {
            ToolError::Execution("No result in tools/call response".to_string())
        })?;

        let call_resp: ToolsCallResponse = serde_json::from_value(result).map_err(|e| {
            ToolError::Execution(format!("Failed to parse tools/call response: {}", e))
        })?;

        // Extract text from content
        let texts: Vec<String> = call_resp
            .content
            .iter()
            .filter_map(|item| item.text.clone())
            .collect();

        Ok(texts.join("\n"))
    }

    /// Create a tool wrapper for dispatching via ToolDispatcher
    pub fn into_dispatcher(self: Arc<Self>) -> McpClientDispatcher {
        McpClientDispatcher {
            client: self,
        }
    }
}

impl Drop for McpClientBridge {
    fn drop(&mut self) {
        if self.child.is_some() {
            // Try to kill the process but ignore errors since we might already be in a drop
            let _ = self.disconnect();
        }
    }
}

/// Wrapper that implements ToolDispatcher for McpClientBridge
pub struct McpClientDispatcher {
    client: Arc<McpClientBridge>,
}

#[async_trait]
impl ToolDispatcher for McpClientDispatcher {
    fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.client.list_tools()
    }

    async fn dispatch(
        &self,
        call: &ToolCall,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        // Extract the remote tool name by removing the namespace prefix
        let full_name = &call.name;
        let remote_name = full_name.strip_prefix(&format!("{}.", self.client.server_name))
            .unwrap_or(full_name);

        self.client.call_tool(remote_name, serde_json::to_value(&call.arguments).unwrap_or(serde_json::Value::Object(Default::default()))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest::new(1, "tools/list", None);
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"tools/list\""));
    }

    #[test]
    fn test_initialize_params_serialization() {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({}),
            client_info: ClientInfo {
                name: "hermes-agent".to_string(),
                version: "0.1.0".to_string(),
            },
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"protocol_version\":\"2024-11-05\""));
        assert!(json.contains("\"client_info\""));
    }

    #[test]
    fn test_remote_tool_deserialization() {
        let json = r#"{
            "name": "create_issue",
            "description": "Create a GitHub issue",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "body": {"type": "string"}
                }
            }
        }"#;

        let tool: RemoteTool = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "create_issue");
        assert_eq!(tool.description, "Create a GitHub issue");
    }

    #[test]
    fn test_tools_call_response_deserialization() {
        let json = r#"{
            "content": [
                {"type": "text", "text": "Issue created: #123"}
            ]
        }"#;

        let response: ToolsCallResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content.len(), 1);
        assert_eq!(response.content[0].item_type, "text");
        assert_eq!(response.content[0].text.as_ref().unwrap(), "Issue created: #123");
    }

    #[test]
    fn test_tools_list_response_deserialization() {
        let json = r#"{
            "tools": [
                {"name": "tool1", "description": "desc1", "inputSchema": {}},
                {"name": "tool2", "description": "desc2", "inputSchema": {}}
            ]
        }"#;

        let response: ToolsListResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.tools.len(), 2);
        assert_eq!(response.tools[0].name, "tool1");
        assert_eq!(response.tools[1].name, "tool2");
    }
}
