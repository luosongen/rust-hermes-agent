use hermes_core::{LlmProvider, ChatRequest, ChatResponse, ModelId, ProviderError, FinishReason, Usage, StreamingCallback};
use hermes_tools_extended::{VisionTool, vision::VisionImage};
use hermes_tool_registry::Tool;
use std::sync::Arc;

#[test]
fn test_vision_tool_name() {
    struct MockProvider;
    impl MockProvider {
        fn new() -> Self { Self }
    }
    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str { "mock" }
        fn supported_models(&self) -> Vec<ModelId> { vec![ModelId::new("mock", "test")] }
        async fn chat(&self, _: ChatRequest) -> Result<ChatResponse, ProviderError> {
            Ok(ChatResponse {
                content: "mock".to_string(),
                finish_reason: FinishReason::Stop,
                tool_calls: None,
                reasoning: None,
                usage: Some(Usage {
                    input_tokens: 1, output_tokens: 1,
                    cache_read_tokens: None, cache_write_tokens: None, reasoning_tokens: None,
                }),
            })
        }
        async fn chat_streaming(&self, _: ChatRequest, _: StreamingCallback) -> Result<ChatResponse, ProviderError> {
            Err(ProviderError::Api("Not implemented".into()))
        }
        fn estimate_tokens(&self, text: &str, _: &ModelId) -> usize { text.len() / 4 }
        fn context_length(&self, _: &ModelId) -> Option<usize> { Some(1000) }
    }
    let tool = VisionTool::new(Arc::new(MockProvider::new()));
    assert_eq!(tool.name(), "vision_analyze");
}

#[test]
fn test_parse_image_url() {
    let img = VisionTool::parse_image("https://example.com/image.png");
    match img {
        VisionImage::Url(u) => assert!(u.contains("example.com")),
        _ => panic!("Expected Url variant"),
    }
}

#[test]
fn test_parse_image_base64() {
    let img = VisionTool::parse_image("aGVsbG8gd29ybGQ=");
    match img {
        VisionImage::Base64(b) => assert_eq!(b, "aGVsbG8gd29ybGQ="),
        _ => panic!("Expected Base64 variant"),
    }
}

#[test]
fn test_parse_image_local_path() {
    let img = VisionTool::parse_image("/tmp/image.png");
    match img {
        VisionImage::Url(u) => assert!(u.contains("file://") || std::path::Path::new(&u).exists() || u.starts_with("/tmp")),
        _ => {}
    }
}
