//! MCP Transport Trait

use async_trait::async_trait;
use std::error::Error;

use crate::protocol::{McpRequest, McpResponse};

/// MCP Transport trait - 抽象传输层
#[async_trait]
pub trait Transport: Send + Sync {
    /// 连接到 MCP 服务器
    async fn connect(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// 断开连接
    async fn disconnect(&mut self) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// 发送请求并接收响应
    async fn send(&mut self, request: McpRequest) -> Result<McpResponse, Box<dyn Error + Send + Sync>>;

    /// 检查是否已连接
    fn is_connected(&self) -> bool;
}