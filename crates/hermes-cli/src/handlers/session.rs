//! 会话命令处理器
//!
//! 提供 `session` 子命令的处理函数：列出、查看、搜索、删除会话。

use anyhow::Result;
use hermes_memory::{SessionStore, SqliteSessionStore};

/// 处理 `session list` 命令，列出所有会话
pub async fn list_sessions() -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    let sessions = store.list_sessions(50, 0).await?;

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    println!("{:40} {:>10} {:>15} {}", "ID", "Messages", "Model", "Started");
    println!("{}", "-".repeat(80));
    for s in sessions {
        println!(
            "{:40} {:>10} {:>15} {}",
            s.id,
            s.message_count,
            s.model.as_deref().unwrap_or("-"),
            s.started_at
        );
    }
    Ok(())
}

/// 处理 `session show` 命令，显示会话详情
pub async fn show_session(id: &str) -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    let session = store.get_session(id).await?;

    match session {
        Some(s) => {
            println!("Session: {}", s.id);
            println!("Source: {}", s.source);
            println!("Model: {:?}", s.model);
            println!("Messages: {}", s.message_count);
            println!("Tool calls: {}", s.tool_call_count);
            println!("Started: {}", s.started_at);
            println!("Ended: {:?}", s.end_reason);
            println!("Input tokens: {}", s.input_tokens);
            println!("Output tokens: {}", s.output_tokens);
        }
        None => {
            println!("Session not found: {}", id);
        }
    }
    Ok(())
}

/// 处理 `session search` 命令，搜索会话内容
pub async fn search_sessions(query: &str) -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    let results = store.search_messages(query, 20).await?;

    if results.is_empty() {
        println!("No results found for: {}", query);
        return Ok(());
    }

    println!("Search results for '{}':", query);
    for r in results {
        println!("\n[session: {}] {}", r.session_id, r.snippet);
    }
    Ok(())
}

/// 处理 `session delete` 命令，删除指定会话
pub async fn delete_session(id: &str) -> Result<()> {
    let store = SqliteSessionStore::new("hermes.db".into()).await?;
    store.delete_session(id).await?;
    println!("Deleted session: {}", id);
    Ok(())
}
