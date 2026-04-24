//! Integration tests for insights module

use hermes_core::{
    insights::{InMemoryInsightsTracker, InsightsTracker, SessionInsights, ToolCallRecord},
    usage_pricing::{CostCalculator, PricingDatabase},
    Usage,
};

#[test]
fn test_insights_tracker_records_tool_calls() {
    let tracker = InMemoryInsightsTracker::new("session1", "openai", "gpt-4o");

    tracker.record_tool_call(ToolCallRecord {
        tool_name: "ReadFile".to_string(),
        started_at: 1000.0,
        duration_ms: 50,
        success: true,
        error: None,
    });

    let insights = tracker.get_insights();
    assert_eq!(insights.tool_calls.len(), 1);
    assert_eq!(insights.tool_calls[0].tool_name, "ReadFile");
}

#[test]
fn test_insights_tracker_records_usage() {
    let tracker = InMemoryInsightsTracker::new("session1", "openai", "gpt-4o");

    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 2000,
        cache_read_tokens: Some(500),
        cache_write_tokens: Some(100),
        reasoning_tokens: None,
    };

    tracker.record_usage(&usage, 0.036625);

    let insights = tracker.get_insights();
    assert_eq!(insights.input_tokens, 1000);
    assert_eq!(insights.output_tokens, 2000);
    assert_eq!(insights.estimated_cost_usd, 0.036625);
}

#[test]
fn test_cost_calculator() {
    let pricing = PricingDatabase::new();
    let calculator = CostCalculator::new(&pricing);

    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 2000,
        cache_read_tokens: None,
        cache_write_tokens: None,
        reasoning_tokens: None,
    };

    let cost = calculator.calculate("openai", "gpt-4o-mini", &usage);
    assert!(cost.is_some());
    // 1000/1M * $0.15 + 2000/1M * $0.60 = $0.00135
    assert!((cost.unwrap() - 0.00135).abs() < 0.0001);
}

#[test]
fn test_session_insights_new() {
    let insights = SessionInsights::new("sess1", "anthropic", "claude-3-5-sonnet-20241022");
    assert_eq!(insights.session_id, "sess1");
    assert_eq!(insights.provider, "anthropic");
    assert_eq!(insights.model, "claude-3-5-sonnet-20241022");
    assert_eq!(insights.input_tokens, 0);
}
