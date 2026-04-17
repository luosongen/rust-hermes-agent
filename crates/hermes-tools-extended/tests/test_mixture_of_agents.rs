use hermes_tool_registry::Tool;

#[test]
fn test_moa_tool_name() {
    // This will panic if OPENROUTER_API_KEY is not set, which is expected
    // Run with OPENROUTER_API_KEY=dummy to test
    unsafe { std::env::set_var("OPENROUTER_API_KEY", "test_key"); }
    let tool = hermes_tools_extended::mixture_of_agents::MixtureOfAgentsTool::new();
    assert_eq!(tool.name(), "mixture_of_agents");
}

#[test]
fn test_moa_params() {
    unsafe { std::env::set_var("OPENROUTER_API_KEY", "test_key"); }
    let tool = hermes_tools_extended::mixture_of_agents::MixtureOfAgentsTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/prompt").is_some());
    assert!(params.pointer("/properties/reference_models").is_some());
    assert!(params.pointer("/properties/aggregator_model").is_some());
}
