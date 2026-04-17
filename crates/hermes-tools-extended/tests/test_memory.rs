use hermes_core::ToolContext;
use hermes_memory::SqliteSessionStore;
use hermes_tools_extended::memory::MemoryTool;
use hermes_tool_registry::Tool;
use std::path::PathBuf;
use std::sync::Arc;

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_directory: PathBuf::from("/tmp"),
        user_id: None,
        task_id: None,
    }
}

fn make_temp_store() -> Arc<SqliteSessionStore> {
    use std::env::temp_dir;
    let temp_path = temp_dir().join(format!("hermes_test_{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    Arc::new(block_on(SqliteSessionStore::new(temp_path)).unwrap())
}

#[test]
fn test_memory_tool_name() {
    let store = make_temp_store();
    let tool = MemoryTool::new(store);
    assert_eq!(tool.name(), "memory");
}

#[test]
fn test_memory_params_set_with_category() {
    let json = serde_json::json!({
        "action": "set",
        "key": "k1",
        "value": "v1",
        "category": "research"
    });
    let params = serde_json::from_value::<hermes_tools_extended::memory::MemoryParams>(json).unwrap();
    match params {
        hermes_tools_extended::memory::MemoryParams::Set { key, value: _, category, tags: _ } => {
            assert_eq!(key, "k1");
            assert_eq!(category, Some("research".to_string()));
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_memory_params_read() {
    let json = serde_json::json!({
        "action": "read",
        "category": "research"
    });
    let params = serde_json::from_value::<hermes_tools_extended::memory::MemoryParams>(json).unwrap();
    match params {
        hermes_tools_extended::memory::MemoryParams::Read { category } => {
            assert_eq!(category, Some("research".to_string()));
        }
        _ => panic!("Expected Read"),
    }
}

#[test]
fn test_memory_params_read_no_filter() {
    let json = serde_json::json!({
        "action": "read"
    });
    let params = serde_json::from_value::<hermes_tools_extended::memory::MemoryParams>(json).unwrap();
    match params {
        hermes_tools_extended::memory::MemoryParams::Read { category } => {
            assert_eq!(category, None);
        }
        _ => panic!("Expected Read"),
    }
}

#[test]
fn test_memory_set_get() {
    let store = make_temp_store();
    let tool = MemoryTool::new(store);

    block_on(async {
        let ctx = make_ctx();
        let result = tool.execute(serde_json::json!({
            "action": "set",
            "key": "test_key",
            "value": "test_value"
        }), ctx.clone()).await;
        assert!(result.is_ok());

        let result = tool.execute(serde_json::json!({
            "action": "get",
            "key": "test_key"
        }), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("test_key"));
        assert!(output.contains("test_value"));
    });
}

#[test]
fn test_memory_search_fts() {
    let store = make_temp_store();
    let tool = MemoryTool::new(store);

    block_on(async {
        let ctx = make_ctx();

        // Insert some data
        tool.execute(serde_json::json!({
            "action": "set",
            "key": "project_alpha",
            "value": "Alpha project notes"
        }), ctx.clone()).await.unwrap();

        tool.execute(serde_json::json!({
            "action": "set",
            "key": "project_beta",
            "value": "Beta project notes"
        }), ctx.clone()).await.unwrap();

        // Search using FTS
        let result = tool.execute(serde_json::json!({
            "action": "search",
            "query": "project"
        }), ctx.clone()).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("project_alpha") || output.contains("Alpha"));

        // Search with category
        tool.execute(serde_json::json!({
            "action": "set",
            "key": "idea1",
            "value": "An innovative idea",
            "category": "brainstorm"
        }), ctx.clone()).await.unwrap();

        let result = tool.execute(serde_json::json!({
            "action": "search",
            "query": "innovative"
        }), ctx).await;
        assert!(result.is_ok());
    });
}

#[test]
fn test_memory_read_by_category() {
    let store = make_temp_store();
    let tool = MemoryTool::new(store);

    block_on(async {
        let ctx = make_ctx();

        // Insert entries with different categories
        tool.execute(serde_json::json!({
            "action": "set",
            "key": "note1",
            "value": "Research notes",
            "category": "research"
        }), ctx.clone()).await.unwrap();

        tool.execute(serde_json::json!({
            "action": "set",
            "key": "note2",
            "value": "Personal notes",
            "category": "personal"
        }), ctx.clone()).await.unwrap();

        tool.execute(serde_json::json!({
            "action": "set",
            "key": "note3",
            "value": "Uncategorized notes"
        }), ctx.clone()).await.unwrap();

        // Read all
        let result = tool.execute(serde_json::json!({
            "action": "read"
        }), ctx.clone()).await;
        assert!(result.is_ok());

        // Read by category
        let result = tool.execute(serde_json::json!({
            "action": "read",
            "category": "research"
        }), ctx.clone()).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("note1") || output.contains("Research"));

        // Read personal only
        let result = tool.execute(serde_json::json!({
            "action": "read",
            "category": "personal"
        }), ctx).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("note2") || output.contains("Personal"));
    });
}

#[test]
fn test_memory_tags() {
    let json = serde_json::json!({
        "action": "set",
        "key": "k1",
        "value": "v1",
        "category": "test",
        "tags": ["rust", "memory", "tool"]
    });
    let params = serde_json::from_value::<hermes_tools_extended::memory::MemoryParams>(json).unwrap();
    match params {
        hermes_tools_extended::memory::MemoryParams::Set { key, value, category, tags } => {
            assert_eq!(key, "k1");
            assert_eq!(value, "v1");
            assert_eq!(category, Some("test".to_string()));
            assert_eq!(tags, vec!["rust".to_string(), "memory".to_string(), "tool".to_string()]);
        }
        _ => panic!("Expected Set"),
    }
}

#[test]
fn test_session_remember_and_search() {
    let store = make_temp_store();
    let tool = MemoryTool::new(store);
    block_on(tool.ensure_fts()).unwrap();

    block_on(async {
        let ctx = make_ctx();

        // 写入 session message
        tool.execute(serde_json::json!({
            "action": "session_remember",
            "session_id": "session-alpha",
            "role": "user",
            "content": "I want to build a Rust CLI tool"
        }), ctx.clone()).await.unwrap();

        tool.execute(serde_json::json!({
            "action": "session_remember",
            "session_id": "session-alpha",
            "role": "assistant",
            "content": "Great! Rust is perfect for CLI tools"
        }), ctx.clone()).await.unwrap();

        // 搜索
        let result = tool.execute(serde_json::json!({
            "action": "session_search",
            "query": "Rust CLI"
        }), ctx.clone()).await.unwrap();

        let output: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(output["results"].is_array());
    });
}
