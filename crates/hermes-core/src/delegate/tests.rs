//! DelegateTool tests.

use super::*;

#[test]
fn test_delegate_params_deserialization() {
    let json = r#"{"goal": "test goal", "context": "some context"}"#;
    let params: DelegateParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.goal, "test goal");
    assert_eq!(params.context, Some("some context".to_string()));
}

#[test]
fn test_delegate_params_default_iterations() {
    let json = r#"{"goal": "simple goal"}"#;
    let params: DelegateParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.max_iterations, DEFAULT_MAX_ITERATIONS);
}

#[test]
fn test_batch_delegate_params() {
    let json = r#"{
        "tasks": [
            {"goal": "task 1"},
            {"goal": "task 2", "max_iterations": 100}
        ],
        "max_concurrent": 5
    }"#;
    let params: BatchDelegateParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.tasks.len(), 2);
    assert_eq!(params.max_concurrent, 5);
    assert_eq!(params.tasks[1].max_iterations, 100);
}

#[test]
fn test_delegate_result_serialization() {
    let result = DelegateResult {
        status: DelegateStatus::Completed,
        summary: "Task done".to_string(),
        api_calls: 5,
        duration_ms: 1234,
        model: "gpt-4o".to_string(),
        exit_reason: "completed".to_string(),
        tool_trace: vec![ToolTraceEntry {
            tool: "Bash".to_string(),
            args_bytes: 20,
            result_bytes: 1042,
            status: "ok".to_string(),
        }],
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"status\":\"completed\""));
    assert!(json.contains("\"api_calls\":5"));
}

#[test]
fn test_batch_delegate_result() {
    let results = vec![
        DelegateResult {
            status: DelegateStatus::Completed,
            summary: "Done".to_string(),
            api_calls: 2,
            duration_ms: 500,
            model: "gpt-4o".to_string(),
            exit_reason: "completed".to_string(),
            tool_trace: vec![],
        },
        DelegateResult {
            status: DelegateStatus::Failed,
            summary: "".to_string(),
            api_calls: 1,
            duration_ms: 300,
            model: "gpt-4o".to_string(),
            exit_reason: "completed".to_string(),
            tool_trace: vec![],
        },
    ];
    let batch = BatchDelegateResult {
        results,
        total_duration_ms: 800,
    };
    let json = serde_json::to_string(&batch).unwrap();
    assert!(json.contains("\"total_duration_ms\":800"));
}

#[test]
fn test_blocked_tools_presence() {
    assert!(BLOCKED_TOOLS.contains(&"delegate"));
    assert!(BLOCKED_TOOLS.contains(&"clarify"));
    assert!(BLOCKED_TOOLS.contains(&"memory"));
    assert!(BLOCKED_TOOLS.contains(&"send_message"));
    assert!(BLOCKED_TOOLS.contains(&"execute_code"));
}

#[test]
fn test_max_depth_constant() {
    assert_eq!(MAX_DELEGATION_DEPTH, 2);
}
