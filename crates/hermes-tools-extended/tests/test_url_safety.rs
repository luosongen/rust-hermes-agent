use hermes_tool_registry::Tool;
use hermes_tools_extended::url_safety::UrlSafetyTool;

#[test]
fn test_url_safety_tool_name() {
    let tool = UrlSafetyTool::new();
    assert_eq!(tool.name(), "url_check");
}

#[test]
fn test_url_safety_parameters() {
    let tool = UrlSafetyTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/url").is_some());
    assert!(params.pointer("/properties/text").is_some());
    assert!(params.pointer("/properties/action").is_some());
}
