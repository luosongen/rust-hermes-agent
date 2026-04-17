use hermes_tool_registry::Tool;

#[test]
fn test_homeassistant_tool_name() {
    let tool = hermes_tools_extended::homeassistant::HomeAssistantTool::new();
    assert_eq!(tool.name(), "homeassistant");
}

#[test]
fn test_homeassistant_params_structure() {
    let tool = hermes_tools_extended::homeassistant::HomeAssistantTool::new();
    let params = tool.parameters();
    assert!(params.pointer("/oneOf").is_some());
    let one_of = params["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 4);
}

#[test]
fn test_homeassistant_entity_id_validation() {
    // Valid entity IDs
    assert!(hermes_tools_extended::homeassistant::validate_entity_id("light.living_room").is_ok());
    assert!(hermes_tools_extended::homeassistant::validate_entity_id("switch.ac_unit").is_ok());

    // Invalid entity IDs (blocked domains)
    assert!(hermes_tools_extended::homeassistant::validate_entity_id("shell_command.test").is_err());
    assert!(hermes_tools_extended::homeassistant::validate_entity_id("python_script.test").is_err());

    // Invalid format
    assert!(hermes_tools_extended::homeassistant::validate_entity_id("not-valid").is_err());
}
