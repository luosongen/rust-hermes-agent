//! MCP Client Implementation

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::protocol::*;
use crate::transport::Transport;

/// MCP 客户端
pub struct McpClient<T: Transport> {
    transport: Arc<RwLock<T>>,
    initialized: Arc<RwLock<bool>>,
    server_info: Arc<RwLock<Option<ServerInfo>>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

impl<T: Transport> McpClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(RwLock::new(transport)),
            initialized: Arc::new(RwLock::new(false)),
            server_info: Arc::new(RwLock::new(None)),
        }
    }

    /// 初始化 MCP 会话
    pub async fn initialize(&mut self) -> Result<(), McpClientError> {
        let request = McpRequest::new("initialize")
            .with_params(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": true},
                "clientInfo": {
                    "name": "hermes-agent",
                    "version": "0.1.0"
                }
            }))
            .with_id(1);

        let response = self.send_request(request).await?;

        // 标记已初始化
        *self.initialized.write().await = true;

        // 提取服务器信息
        if let Some(result) = response.result {
            if let Ok(info) = serde_json::from_value::<ServerInfo>(result.clone()) {
                *self.server_info.write().await = Some(info);
            }
        }

        Ok(())
    }

    /// 列出可用工具
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McpClientError> {
        self.ensure_initialized().await?;

        let request = McpRequest::new("tools/list").with_id(2);
        let response = self.send_request(request).await?;

        match response.result {
            Some(result) => {
                let list_result: ListToolsResult =
                    serde_json::from_value(result).map_err(|e| McpClientError::ParseError(e.to_string()))?;
                Ok(list_result.tools)
            }
            None => Err(McpClientError::NoResult),
        }
    }

    /// 调用工具
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpClientError> {
        self.ensure_initialized().await?;

        let request = McpRequest::new("tools/call")
            .with_params(serde_json::json!({
                "name": name,
                "arguments": arguments
            }))
            .with_id(3);

        let response = self.send_request(request).await?;

        match response.result {
            Some(result) => {
                serde_json::from_value(result).map_err(|e| McpClientError::ParseError(e.to_string()))
            }
            None => Err(McpClientError::NoResult),
        }
    }

    async fn send_request(&self, request: McpRequest) -> Result<McpResponse, McpClientError> {
        let mut transport = self.transport.write().await;
        transport.send(request).await.map_err(|e| McpClientError::Transport(e.to_string()))
    }

    async fn ensure_initialized(&self) -> Result<(), McpClientError> {
        if !*self.initialized.read().await {
            return Err(McpClientError::NotInitialized);
        }
        Ok(())
    }

    /// 获取服务器信息
    pub async fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().await.clone()
    }

    /// 检查是否已初始化
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
}

/// MCP 客户端错误
#[derive(Debug, thiserror::Error)]
pub enum McpClientError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Not initialized, call initialize() first")]
    NotInitialized,
    #[error("No result in response")]
    NoResult,
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Server error: {0}")]
    Server(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_error_display() {
        let err = McpClientError::NotInitialized;
        assert!(err.to_string().contains("Not initialized"));
    }
}