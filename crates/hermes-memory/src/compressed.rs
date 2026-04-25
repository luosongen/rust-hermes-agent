//! Compressed message segment structure

use chrono::{DateTime, Utc};

/// A compressed segment of messages
#[derive(Debug, Clone)]
pub struct CompressedSegment {
    pub id: String,
    pub session_id: String,
    pub start_message_id: i64,
    pub end_message_id: i64,
    pub summary: String,
    pub vector: Vec<f32>,
    pub created_at: DateTime<Utc>,
}

impl CompressedSegment {
    pub fn new(
        session_id: String,
        start_message_id: i64,
        end_message_id: i64,
        summary: String,
        vector: Vec<f32>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            start_message_id,
            end_message_id,
            summary,
            vector,
            created_at: Utc::now(),
        }
    }

    /// Get the range of message IDs covered by this segment
    pub fn message_range(&self) -> (i64, i64) {
        (self.start_message_id, self.end_message_id)
    }

    /// Check if a message ID falls within this segment
    pub fn contains(&self, message_id: i64) -> bool {
        message_id >= self.start_message_id && message_id <= self.end_message_id
    }
}