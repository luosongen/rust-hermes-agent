use hermes_tool_registry::Tool;
use hermes_tools_extended::text_to_speech::{TextToSpeechTool, TtsParams};

#[test]
fn test_tts_tool_name() {
    let tool = TextToSpeechTool::new();
    assert_eq!(tool.name(), "text_to_speech");
}

#[test]
fn test_tts_params_defaults() {
    let params: TtsParams = serde_json::from_value(serde_json::json!({
        "text": "Hello world"
    })).unwrap();
    assert_eq!(params.provider, "edge-tts");
    assert_eq!(params.voice, "zh-CN-XiaoxiaoNeural");
    assert_eq!(params.model, "tts-1");
    assert_eq!(params.output_path, "output.mp3");
}

#[test]
fn test_tts_params_custom() {
    let params: TtsParams = serde_json::from_value(serde_json::json!({
        "text": "Hello",
        "provider": "openai",
        "voice": "alloy",
        "model": "tts-1-hd",
        "output_path": "/tmp/test.mp3"
    })).unwrap();
    assert_eq!(params.provider, "openai");
    assert_eq!(params.voice, "alloy");
    assert_eq!(params.model, "tts-1-hd");
}

#[test]
fn test_tts_tool_parameters_schema() {
    let tool = TextToSpeechTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/text").is_some());
    assert!(params.pointer("/properties/provider").is_some());
    assert!(params.pointer("/properties/voice").is_some());
    assert!(params.pointer("/properties/output_path").is_some());
}
