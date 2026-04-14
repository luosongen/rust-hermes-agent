//! SQLite 会话存储测试
//!
//! 测试 SqliteSessionStore 的 CRUD 操作

use hermes_memory::{NewMessage, NewSession, SessionStore, SqliteSessionStore};
use std::time::SystemTime;
use tempfile::tempdir;

/// 获取当前时间戳（UTC 秒数）
fn now() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

/// 测试创建会话并获取
#[tokio::test]
async fn test_create_and_get_session() {
    // 创建临时目录用于测试数据库
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = SqliteSessionStore::new(db_path).await.unwrap();

    // 创建新会话
    let new_session = NewSession {
        id: "test-session-1".to_string(),
        source: "test".to_string(),
        user_id: Some("user123".to_string()),
        model: Some("openai/gpt-4o".to_string()),
    };

    // 验证会话创建成功
    let created = store.create_session(new_session.clone()).await.unwrap();
    assert_eq!(created.id, "test-session-1");
    assert_eq!(created.source, "test");
    assert_eq!(created.model, Some("openai/gpt-4o".to_string()));
    assert_eq!(created.message_count, 0);

    // 验证可以从存储中获取会话
    let fetched = store.get_session("test-session-1").await.unwrap();
    assert!(fetched.is_some());
    let session = fetched.unwrap();
    assert_eq!(session.id, "test-session-1");
}

/// 测试获取不存在的会话
#[tokio::test]
async fn test_get_nonexistent_session() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = SqliteSessionStore::new(db_path).await.unwrap();

    let result = store.get_session("nonexistent").await.unwrap();
    assert!(result.is_none());
}

/// 测试追加消息
#[tokio::test]
async fn test_append_message() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = SqliteSessionStore::new(db_path).await.unwrap();

    // 先创建会话
    let session = NewSession {
        id: "test-session-2".to_string(),
        source: "test".to_string(),
        user_id: None,
        model: None,
    };
    store.create_session(session).await.unwrap();

    // 追加消息
    let message = NewMessage {
        role: "user".to_string(),
        content: Some("Hello".to_string()),
        tool_call_id: None,
        tool_calls: None,
        tool_name: None,
        timestamp: now(),
        token_count: None,
        finish_reason: None,
        reasoning: None,
    };

    // 验证消息追加成功
    let appended = store.append_message("test-session-2", message).await.unwrap();
    assert_eq!(appended.content, Some("Hello".to_string()));
    assert_eq!(appended.role, "user");

    // 验证会话的消息计数已更新
    let session = store.get_session("test-session-2").await.unwrap().unwrap();
    assert_eq!(session.message_count, 1);
}

/// 测试获取消息（分页）
#[tokio::test]
async fn test_get_messages() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = SqliteSessionStore::new(db_path).await.unwrap();

    // 创建会话
    let session = NewSession {
        id: "test-session-3".to_string(),
        source: "test".to_string(),
        user_id: None,
        model: None,
    };
    store.create_session(session).await.unwrap();

    // 添加 5 条消息
    for i in 0..5 {
        let msg = NewMessage {
            role: "user".to_string(),
            content: Some(format!("Message {}", i)),
            tool_call_id: None,
            tool_calls: None,
            tool_name: None,
            timestamp: now(),
            token_count: None,
            finish_reason: None,
            reasoning: None,
        };
        store.append_message("test-session-3", msg).await.unwrap();
    }

    // 验证 limit 功能
    let messages = store.get_messages("test-session-3", 3, 0).await.unwrap();
    assert_eq!(messages.len(), 3);

    // 验证 offset 功能（分页）
    let messages = store.get_messages("test-session-3", 3, 3).await.unwrap();
    assert_eq!(messages.len(), 2);
}

/// 测试多个会话隔离
#[tokio::test]
async fn test_multiple_sessions() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let store = SqliteSessionStore::new(db_path).await.unwrap();

    // 创建 3 个独立会话
    for i in 0..3 {
        let session = NewSession {
            id: format!("session-{}", i),
            source: "test".to_string(),
            user_id: None,
            model: None,
        };
        store.create_session(session).await.unwrap();
    }

    // 验证所有会话都能独立获取
    for i in 0..3 {
        let result = store.get_session(&format!("session-{}", i)).await.unwrap();
        assert!(result.is_some());
    }
}
