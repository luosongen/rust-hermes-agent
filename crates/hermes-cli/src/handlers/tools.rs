//! 工具命令处理器
//!
//! 提供工具的列出、启用和禁用命令。

use anyhow::Result;
use hermes_core::config::Config;
use hermes_environment::LocalEnvironment;
use hermes_tool_registry::ToolRegistry;
use hermes_tools_builtin::register_builtin_tools;
use std::sync::Arc;

/// 列出所有已注册的工具
pub fn list_tools() -> Result<()> {
    let registry = Arc::new(ToolRegistry::new());
    let environment = Arc::new(LocalEnvironment::new("."));
    register_builtin_tools(&registry, environment);

    let tools = registry.tool_names();
    println!("Available tools:\n");
    for tool in tools {
        println!("  {}", tool);
    }
    Ok(())
}

/// 在配置中启用指定工具
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

/// 在配置中禁用指定工具
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
