use hermes_tools_extended::delegate_tool::{DelegateTool, DelegateParams};
use hermes_tool_registry::Tool;
use std::path::PathBuf;

#[test]
fn test_delegate_tool_name() {
    let tool = DelegateTool::new(
        PathBuf::from("/usr/local/bin/hermes"),
        PathBuf::from("/tmp")
    );
    assert_eq!(tool.name(), "delegate_task");
}

#[test]
fn test_delegate_params_deserialization() {
    let json = serde_json::json!({
        "goal": "Search for rust async runtime info",
        "toolsets": ["web"],
        "max_iterations": 50
    });
    let params: DelegateParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.goal, "Search for rust async runtime info");
    assert_eq!(params.toolsets, vec!["web"]);
    assert_eq!(params.max_iterations, Some(50));
}

#[test]
fn test_delegate_params_minimal() {
    let json = serde_json::json!({
        "goal": "Simple task",
        "toolsets": ["read"]
    });
    let params: DelegateParams = serde_json::from_value(json).unwrap();
    assert_eq!(params.goal, "Simple task");
    assert_eq!(params.toolsets, vec!["read"]);
    assert_eq!(params.max_iterations, None);
    assert_eq!(params.context, None);
}
