use hermes_tool_registry::Tool;

#[test]
fn test_image_generation_tool_name() {
    let tool = hermes_tools_extended::image_generation::ImageGenerationTool::new();
    assert_eq!(tool.name(), "image_generate");
}

#[test]
fn test_image_generation_params() {
    let tool = hermes_tools_extended::image_generation::ImageGenerationTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/properties/prompt").is_some());
    assert!(params.pointer("/properties/image_size").is_some());
    assert!(params.pointer("/properties/num_inference_steps").is_some());
    assert!(params.pointer("/properties/guidance_scale").is_some());
    assert!(params.pointer("/properties/num_images").is_some());
}

#[test]
fn test_image_size_serialization() {
    use hermes_tools_extended::image_generation::ImageSize;
    let size = ImageSize::Landscape16x9;
    let json = serde_json::to_string(&size).unwrap();
    assert_eq!(json, "\"landscape_16_9\"");
}
