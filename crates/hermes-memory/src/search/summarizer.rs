//! LLM-based session summarization

use crate::session::SessionStore;
use std::sync::Arc;

pub struct SessionSummarizer<S: SessionStore> {
    session_store: Arc<S>,
}

impl<S: SessionStore> SessionSummarizer<S> {
    pub fn new(session_store: Arc<S>) -> Self {
        Self { session_store }
    }

    pub async fn summarize_session(
        &self,
        session_id: &str,
        query: &str,
        max_chars: usize,
    ) -> Result<String, String> {
        let messages = self.session_store.get_messages(session_id)
            .map_err(|e| e.to_string())?;

        let truncated = self.truncate_around_query(&messages, query, max_chars);
        let summary_prompt = format!(
            "Summarize this conversation relevant to: '{}'\n\n{}",
            query, truncated
        );

        Ok(truncated)
    }

    fn truncate_around_query(&self, messages: &[crate::session::Message], query: &str, max_chars: usize) -> String {
        let query_lower = query.to_lowercase();
        let mut positions: Vec<usize> = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            if msg.content.to_lowercase().contains(&query_lower) {
                positions.push(i);
            }
        }

        if positions.is_empty() {
            let content: String = messages.iter()
                .take(10)
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            return content.chars().take(max_chars).collect();
        }

        let center = positions[0];
        let mut result = String::new();

        for msg in messages.iter().skip(center.saturating_sub(1)) {
            if result.len() + msg.content.len() > max_chars {
                result.push_str("...[truncated]...");
                break;
            }
            result.push_str(&msg.content);
            result.push('\n');
        }

        result
    }
}