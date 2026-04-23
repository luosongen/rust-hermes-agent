//! LLM-based session summarization
//!
//! # Note
//! This is currently a stub implementation. The `summarize_session` method
//! returns truncated context rather than an LLM-generated summary. Full LLM
//! integration is planned - see Task 7 roadmap.

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
        // TODO: Integrate with LLM provider for actual summarization
        // The summary_prompt below shows the intended prompt structure:
        let messages = self.session_store
            .get_messages(session_id, usize::MAX, 0)
            .await
            .map_err(|e| e.to_string())?;

        let truncated = self.truncate_around_query(&messages, query, max_chars);
        let _summary_prompt = format!(
            "Summarize this conversation relevant to: '{}'\n\n{}",
            query, truncated
        );

        // STUB: Return truncated content instead of LLM summary
        Ok(truncated)
    }

    fn truncate_around_query(&self, messages: &[crate::session::Message], query: &str, max_chars: usize) -> String {
        let query_lower = query.to_lowercase();
        let mut positions: Vec<usize> = Vec::new();

        for (i, msg) in messages.iter().enumerate() {
            if let Some(content) = &msg.content {
                if content.to_lowercase().contains(&query_lower) {
                    positions.push(i);
                }
            }
        }

        if positions.is_empty() {
            let content: String = messages.iter()
                .take(10)
                .filter_map(|m| m.content.as_ref())
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            return content.chars().take(max_chars).collect();
        }

        let center = positions[0];
        let mut result = String::new();

        for msg in messages.iter().skip(center.saturating_sub(1)) {
            let content = match msg.content.as_ref() {
                Some(c) => c,
                None => continue,
            };
            if result.len() + content.len() > max_chars {
                result.push_str("...[truncated]...");
                break;
            }
            result.push_str(content);
            result.push('\n');
        }

        result
    }
}