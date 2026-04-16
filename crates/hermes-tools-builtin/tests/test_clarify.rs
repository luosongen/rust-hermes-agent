use hermes_tool_registry::Tool;
use hermes_tools_builtin::clarify_tools::{AskUserFn, ClarifyTool};
use serde_json::json;

fn make_ask_user_fn() -> AskUserFn {
    Box::new(|question: String, _choices: Option<Vec<String>>| -> String {
        if question.contains("favorite") {
            "B".to_string()
        } else {
            "user answer".to_string()
        }
    })
}

#[test]
fn test_clarify_tool_name() {
    let tool = ClarifyTool::new(make_ask_user_fn());
    assert_eq!(tool.name(), "clarify");
}

#[test]
fn test_clarify_tool_parameters_schema() {
    let tool = ClarifyTool::new(make_ask_user_fn());
    let params = tool.parameters();
    assert!(params.get("properties").is_some());
    let props = params.get("properties").unwrap().as_object().unwrap();
    assert!(props.contains_key("question"));
    assert!(props.contains_key("choices"));
}

#[test]
fn test_clarify_tool_execute_with_question() {
    let ask_user = make_ask_user_fn();
    let tool = ClarifyTool::new(ask_user);
    let args = json!({ "question": "What is your favorite color?" });
    let result = tool.execute_sync(args).unwrap();
    assert!(result.contains("What is your favorite color"));
    assert!(result.contains("user_response"));
    assert!(result.contains("B"));
}

#[test]
fn test_clarify_tool_execute_empty_question() {
    let tool = ClarifyTool::new(make_ask_user_fn());
    let args = json!({ "question": "" });
    let result = tool.execute_sync(args);
    assert!(result.is_err());
}

#[test]
fn test_clarify_tool_execute_with_choices() {
    let ask_user = Box::new(|_q: String, choices: Option<Vec<String>>| -> String {
        choices.map(|c| c.join(",")).unwrap_or_default()
    });
    let tool = ClarifyTool::new(ask_user);
    let args = json!({ "question": "Pick one", "choices": ["A", "B", "C"] });
    let result = tool.execute_sync(args).unwrap();
    assert!(result.contains("choices_offered"));
    assert!(result.contains("A"));
}

#[test]
fn test_clarify_tool_execute_no_callback() {
    let tool = ClarifyTool::new_noop();
    let args = json!({ "question": "Hello?" });
    let result = tool.execute_sync(args).unwrap();
    assert!(result.contains("not available"));
}
