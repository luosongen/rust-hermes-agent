//! 工具注册表测试
//!
//! 测试 ToolRegistry 的注册、获取、分发功能

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolDispatcher, ToolError};
use hermes_tool_registry::{Tool, ToolRegistry};
use std::collections::HashMap;

/// 测试用工具实现
struct TestTool {
    name: String,
    description: String,
    should_fail: bool,
}

impl TestTool {
    fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            should_fail: false,
        }
    }
}

#[async_trait]
impl Tool for TestTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": { "type": "string" }
            }
        })
    }

    async fn execute(
        &self,
        _args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        if self.should_fail {
            Err(ToolError::Execution("Tool failed".to_string()))
        } else {
            Ok(format!("Executed {}", self.name))
        }
    }
}

/// 测试注册和获取工具
#[tokio::test]
async fn test_register_and_get() {
    let registry = ToolRegistry::new();
    registry.register(TestTool::new("test_tool", "A test tool"));

    let tool = registry.get("test_tool");
    assert!(tool.is_some());
    assert_eq!(tool.unwrap().name(), "test_tool");
}

/// 测试注销工具
#[tokio::test]
async fn test_unregister() {
    let registry = ToolRegistry::new();
    registry.register(TestTool::new("removable", "Will be removed"));

    let removed = registry.unregister("removable");
    assert!(removed.is_some());

    let tool = registry.get("removable");
    assert!(tool.is_none());
}

/// 测试获取工具名称列表
#[tokio::test]
async fn test_tool_names() {
    let registry = ToolRegistry::new();
    registry.register(TestTool::new("tool_a", "First"));
    registry.register(TestTool::new("tool_b", "Second"));

    let names = registry.tool_names();
    assert!(names.contains(&"tool_a".to_string()));
    assert!(names.contains(&"tool_b".to_string()));
    assert_eq!(names.len(), 2);
}

/// 测试获取工具定义
#[tokio::test]
async fn test_get_tool_definitions() {
    let registry = ToolRegistry::new();
    registry.register(TestTool::new("def_tool", "Has definitions"));

    let defs = registry.get_tool_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "def_tool");
    assert_eq!(defs[0].description, "Has definitions");
}

/// 测试 ToolDispatcher 的 dispatch 功能
#[tokio::test]
async fn test_tool_dispatcher_dispatch() {
    let registry = ToolRegistry::new();
    registry.register(TestTool::new("dispatch_tool", "Can be dispatched"));

    let call = hermes_core::ToolCall {
        id: "call-1".to_string(),
        name: "dispatch_tool".to_string(),
        arguments: HashMap::new(),
    };
    let context = ToolContext {
        session_id: "test-session".to_string(),
        user_id: None,
        working_directory: std::path::PathBuf::from("."),
        task_id: None,
    };

    let result = registry.dispatch(&call, context).await;
    assert!(result.is_ok());
}

/// 测试 ToolDispatcher 找不到工具时返回错误
#[tokio::test]
async fn test_tool_dispatcher_not_found() {
    let registry = ToolRegistry::new();

    let call = hermes_core::ToolCall {
        id: "call-2".to_string(),
        name: "nonexistent".to_string(),
        arguments: HashMap::new(),
    };
    let context = ToolContext {
        session_id: "test-session".to_string(),
        user_id: None,
        working_directory: std::path::PathBuf::from("."),
        task_id: None,
    };

    let result = registry.dispatch(&call, context).await;
    assert!(result.is_err());
}
