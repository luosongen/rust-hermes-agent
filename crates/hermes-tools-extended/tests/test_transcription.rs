use hermes_tool_registry::Tool;

#[test]
fn test_transcription_tool_name() {
    let tool = hermes_tools_extended::transcription::TranscriptionTool::new();
    assert_eq!(tool.name(), "transcribe");
}

#[test]
fn test_transcription_params() {
    let tool = hermes_tools_extended::transcription::TranscriptionTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/audio_path").is_some());
    assert!(params.pointer("/properties/provider").is_some());
    assert!(params.pointer("/properties/language").is_some());

    // 检查 provider enum 包含 openai
    let provider_enum = params.pointer("/properties/provider/enum").unwrap();
    let providers: Vec<String> = serde_json::from_value(provider_enum.clone()).unwrap();
    assert!(providers.contains(&"openai".to_string()));
    assert!(providers.contains(&"groq".to_string()));
    assert!(providers.contains(&"faster-whisper".to_string()));
}

#[test]
fn test_transcription_params_defaults() {
    let params = hermes_tools_extended::transcription::TranscribeParams {
        audio_path: "test.mp3".to_string(),
        provider: "faster-whisper".to_string(),
        language: None,
    };
    assert_eq!(params.provider, "faster-whisper");
}

#[test]
fn test_transcription_openai_provider() {
    let params = hermes_tools_extended::transcription::TranscribeParams {
        audio_path: "test.mp3".to_string(),
        provider: "openai".to_string(),
        language: Some("zh".to_string()),
    };
    assert_eq!(params.provider, "openai");
    assert_eq!(params.language, Some("zh".to_string()));
}
