use hermes_tools_extended::web_search::{WebSearchTool, SearchResult};
use hermes_tool_registry::Tool;

#[test]
fn test_web_search_tool_name() {
    let tool = WebSearchTool::new();
    assert_eq!(tool.name(), "web_search");
}

#[test]
fn test_web_search_default_provider() {
    let tool = WebSearchTool::new();
    assert!(tool.providers.contains_key("duckduckgo"));
}

#[test]
fn test_web_search_provider_names() {
    let tool = WebSearchTool::new()
        .with_exa("test-key".to_string())
        .with_tavily("test-key".to_string())
        .with_firecrawl("test-key".to_string(), "search");

    assert!(tool.providers.contains_key("duckduckgo"));
    assert!(tool.providers.contains_key("exa"));
    assert!(tool.providers.contains_key("tavily"));
    assert!(tool.providers.contains_key("firecrawl"));
}

#[test]
fn test_search_result_serialization() {
    let result = SearchResult {
        url: "https://example.com".to_string(),
        title: "Example".to_string(),
        snippet: "An example page".to_string(),
        content: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("example.com"));
}
