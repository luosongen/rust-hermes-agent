//! Tools commands implementation
//!
//! Provides commands for listing, enabling, and disabling tools.

use anyhow::Result;
use hermes_core::config::Config;
use hermes_tool_registry::ToolRegistry;
use hermes_tools_builtin::register_builtin_tools;
use std::sync::Arc;

/// List all registered tools
pub fn list_tools() -> Result<()> {
    let registry = Arc::new(ToolRegistry::new());
    register_builtin_tools(&registry);

    let tools = registry.tool_names();
    println!("Available tools:\n");
    for tool in tools {
        println!("  {}", tool);
    }
    Ok(())
}

/// Enable a tool in config
pub fn enable_tool(tool: &str) -> Result<()> {
    let mut config = Config::load()?;
    let key = format!("tools.{}.enabled", tool);
    if config.set(&key, true.to_string()) {
        config.save()?;
        println!("Tool '{}' enabled", tool);
    } else {
        anyhow::bail!("Failed to enable tool '{}': unsupported config key", tool);
    }
    Ok(())
}

/// Disable a tool in config
pub fn disable_tool(tool: &str) -> Result<()> {
    let mut config = Config::load()?;
    let key = format!("tools.{}.enabled", tool);
    if config.set(&key, false.to_string()) {
        config.save()?;
        println!("Tool '{}' disabled", tool);
    } else {
        anyhow::bail!("Failed to disable tool '{}': unsupported config key", tool);
    }
    Ok(())
}
