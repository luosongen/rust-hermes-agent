//! Integration tests for compression and routing modules.

use std::sync::Arc;
use async_trait::async_trait;

use hermes_core::{
    ChatRequest, ChatResponse, Content, FinishReason, Message, ModelId,
    ProviderError, Role, Usage,
};
use hermes_core::traits::ContextEngine;
use hermes_core::routing::ComplexityDetector;
use hermes_core::compression::ToolResultPruner;

struct MockLlmProvider;

impl MockLlmProvider {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl hermes_core::LlmProvider for MockLlmProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![ModelId::new("mock", "test")]
    }

    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        Ok(ChatResponse {
            content: "Mock summary content".to_string(),
            finish_reason: FinishReason::Stop,
            tool_calls: None,
            reasoning: None,
            usage: Some(Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
            }),
        })
    }

    async fn chat_streaming(
        &self,
        _request: ChatRequest,
        _callback: hermes_core::StreamingCallback,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::Api("Not implemented".into()))
    }

    fn estimate_tokens(&self, text: &str, _model: &ModelId) -> usize {
        text.len() / 4
    }

    fn context_length(&self, _model: &ModelId) -> Option<usize> {
        Some(1000)
    }
}

// =============================================================================
// ComplexityDetector Tests
// =============================================================================

#[test]
fn test_complexity_detector_simple_message() {
    let detector = ComplexityDetector::default();
    assert!(detector.is_simple("Hello, how are you?"));
}

#[test]
fn test_complexity_detector_complex_keyword() {
    let detector = ComplexityDetector::default();
    assert!(!detector.is_simple("Debug this error in my code"));
}

#[test]
fn test_complexity_detector_with_url() {
    let detector = ComplexityDetector::default();
    assert!(!detector.is_simple("Check https://example.com for info"));
}

#[test]
fn test_complexity_detector_with_code() {
    let detector = ComplexityDetector::default();
    assert!(!detector.is_simple("Use `let x = 5;` to define"));
}

// =============================================================================
// ToolResultPruner Tests
// =============================================================================

#[test]
fn test_tool_result_pruner_replaces_tool_content() {
    let pruner = ToolResultPruner::default();
    let placeholder = hermes_core::compression::PRUNED_TOOL_PLACEHOLDER;

    let messages = vec![
        Message::user(Content::Text("Run command".into())),
        Message::assistant(Content::Text("Running...".into())),
        Message {
            role: Role::Tool,
            content: Content::ToolResult {
                tool_call_id: "call_1".into(),
                content: "Long output".into(),
            },
            reasoning: None,
            tool_call_id: Some("call_1".into()),
            tool_name: Some("test".into()),
        },
    ];

    let result = pruner.prune(messages);
    assert_eq!(result.len(), 3);

    if let Content::ToolResult { content, .. } = &result[2].content {
        assert_eq!(content, placeholder);
    } else {
        panic!("Expected ToolResult");
    }
}

#[test]
fn test_tool_result_pruner_preserves_non_tool() {
    let pruner = ToolResultPruner::default();

    let messages = vec![
        Message::user(Content::Text("Hello".into())),
        Message::assistant(Content::Text("Hi!".into())),
    ];

    let result = pruner.prune(messages);
    assert_eq!(result.len(), 2);
}

// =============================================================================
// ContextCompressor Tests
// =============================================================================

#[tokio::test]
async fn test_context_compressor_threshold() {
    use hermes_core::ContextCompressor;

    let llm = Arc::new(MockLlmProvider::new());
    let compressor = ContextCompressor::new(llm, "test".to_string(), 1000);

    // 400 tokens should not trigger (below 50% threshold of 500)
    assert!(!compressor.should_compress(400));
    // 500 tokens should trigger
    assert!(compressor.should_compress(500));
}

#[tokio::test]
async fn test_context_compressor_get_status() {
    use hermes_core::ContextCompressor;

    let llm = Arc::new(MockLlmProvider::new());
    let compressor = ContextCompressor::new(llm, "test-model".to_string(), 1000);

    let status = compressor.get_status();
    assert_eq!(status.threshold_tokens, 500);
    assert_eq!(status.model, "test-model");
    assert_eq!(status.compression_count, 0);
}
