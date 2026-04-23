use serde::{Deserialize, Serialize};

/// MCP transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", content = "config")]
pub enum McpTransport {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        args: Vec<String>,
    },
    #[serde(rename = "http")]
    Http {
        url: String,
    },
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default = "default_mcp_enabled")]
    pub enabled: bool,
    pub transport: McpTransport,
}

fn default_mcp_enabled() -> bool { true }

/// MCP servers configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServersConfig {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}
