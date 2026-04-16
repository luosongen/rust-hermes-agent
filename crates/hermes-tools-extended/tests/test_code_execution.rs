use hermes_tool_registry::Tool;
use hermes_tools_extended::code_execution::{CodeExecutionTool, ExecutionConfig};

#[test]
fn test_code_execution_tool_name() {
    let tool = CodeExecutionTool::new(ExecutionConfig::default());
    assert_eq!(tool.name(), "execute_code");
}

#[test]
fn test_execution_config_defaults() {
    let config = ExecutionConfig::default();
    assert_eq!(config.timeout_secs, 300);
    assert_eq!(config.max_tool_calls, 50);
    assert!(config.allowed_tools.contains(&"read_file".to_string()));
}

#[test]
fn test_generate_stub_uds() {
    let tool = CodeExecutionTool::new(ExecutionConfig::default());
    let stub = tool.generate_stub("uds", Some("/tmp/test.sock"));
    assert!(stub.contains("socket.AF_UNIX"));
    assert!(stub.contains("/tmp/test.sock"));
}

#[test]
fn test_generate_stub_file() {
    let tool = CodeExecutionTool::new(ExecutionConfig::default());
    let stub = tool.generate_stub("file", None);
    assert!(stub.contains("req_file"));
    assert!(stub.contains("resp_file"));
}
