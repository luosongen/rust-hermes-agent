use hermes_tools_builtin::todo_tools::{TodoItem, TodoStore};

#[test]
fn test_todo_store_write_replace() {
    let mut store = TodoStore::new();
    let items = vec![
        TodoItem {
            id: "1".into(),
            content: "Task 1".into(),
            status: "pending".into(),
        },
        TodoItem {
            id: "2".into(),
            content: "Task 2".into(),
            status: "in_progress".into(),
        },
    ];
    let result = store.write(items, false);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, "1");
}

#[test]
fn test_todo_store_write_merge() {
    let mut store = TodoStore::new();
    let initial = vec![TodoItem {
        id: "1".into(),
        content: "Task 1".into(),
        status: "pending".into(),
    }];
    store.write(initial, false);

    let update = vec![TodoItem {
        id: "1".into(),
        content: "Task 1 updated".into(),
        status: "completed".into(),
    }];
    let result = store.write(update, true);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].status, "completed");
}

#[test]
fn test_todo_store_read_empty() {
    let mut store = TodoStore::new();
    let result = store.read();
    assert!(result.is_empty());
}

#[test]
fn test_todo_store_invalid_status_defaults_to_pending() {
    let mut store = TodoStore::new();
    let items = vec![TodoItem {
        id: "1".into(),
        content: "Task".into(),
        status: "invalid".into(),
    }];
    let result = store.write(items, false);
    assert_eq!(result[0].status, "pending");
}

#[test]
fn test_todo_store_empty_id_defaults_to_question_mark() {
    let mut store = TodoStore::new();
    let items = vec![TodoItem {
        id: "".into(),
        content: "Task".into(),
        status: "pending".into(),
    }];
    let result = store.write(items, false);
    assert_eq!(result[0].id, "?");
}

#[test]
fn test_todo_store_dedupe_by_id() {
    let mut store = TodoStore::new();
    let items = vec![
        TodoItem {
            id: "1".into(),
            content: "First".into(),
            status: "pending".into(),
        },
        TodoItem {
            id: "1".into(),
            content: "Second".into(),
            status: "completed".into(),
        },
    ];
    let result = store.write(items, false);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "Second"); // last occurrence wins
}

#[test]
fn test_todo_store_summary_counts() {
    let mut store = TodoStore::new();
    let items = vec![
        TodoItem {
            id: "1".into(),
            content: "p".into(),
            status: "pending".into(),
        },
        TodoItem {
            id: "2".into(),
            content: "i".into(),
            status: "in_progress".into(),
        },
        TodoItem {
            id: "3".into(),
            content: "c".into(),
            status: "completed".into(),
        },
        TodoItem {
            id: "4".into(),
            content: "x".into(),
            status: "cancelled".into(),
        },
    ];
    store.write(items, false);
    let summary = store.summary();
    assert_eq!(summary.pending, 1);
    assert_eq!(summary.in_progress, 1);
    assert_eq!(summary.completed, 1);
    assert_eq!(summary.cancelled, 1);
}
