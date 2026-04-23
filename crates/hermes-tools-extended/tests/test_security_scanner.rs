use hermes_tool_registry::Tool;
use hermes_tools_extended::security_scanner::SecurityScannerTool;

#[test]
fn test_security_scanner_tool_name() {
    let tool = SecurityScannerTool::new();
    assert_eq!(tool.name(), "security_scan");
}

#[test]
fn test_security_scanner_parameters() {
    let tool = SecurityScannerTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/text").is_some());
    assert!(params.pointer("/properties/file_path").is_some());
    assert!(params.pointer("/properties/action").is_some());
}
