//! Context compression manager

use crate::compressed::CompressedSegment;
use crate::compression_config::CompressionConfig;
use crate::compression_error::CompressionError;
use crate::session::SessionStore;
use crate::summarizer::Summarizer;
use std::sync::Arc;

/// 每个压缩段落的最大消息数
const MAX_MESSAGES_PER_SEGMENT: usize = 20;

/// CompressionManager - Manages context compression for sessions
pub struct CompressionManager<S: SessionStore> {
    config: CompressionConfig,
    summarizer: Summarizer,
    store: Arc<S>,
}

impl<S: SessionStore> CompressionManager<S> {
    pub fn new(
        config: CompressionConfig,
        summarizer: Summarizer,
        store: Arc<S>,
    ) -> Self {
        Self {
            config,
            summarizer,
            store,
        }
    }

    /// Check if compression should be triggered
    pub async fn should_compress(&self, session_id: &str) -> Result<bool, CompressionError> {
        if !self.config.enabled {
            tracing::debug!(session_id, "compression disabled");
            return Ok(false);
        }

        // Check message count threshold
        let messages = self.store
            .get_messages(session_id, usize::MAX, 0)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        let message_count = messages.len();
        tracing::debug!(session_id, message_count, threshold = self.config.message_count_threshold, "checking message count threshold");

        if message_count < self.config.message_count_threshold {
            return Ok(false);
        }

        // Check token threshold (simplified - sum token counts)
        let total_tokens: usize = messages
            .iter()
            .filter_map(|m| m.token_count)
            .sum();

        tracing::debug!(session_id, total_tokens, threshold = self.config.token_threshold, "checking token threshold");

        if total_tokens < self.config.token_threshold {
            return Ok(false);
        }

        tracing::info!(session_id, message_count, total_tokens, "compression should be triggered");
        Ok(true)
    }

    /// Compress messages for a session
    pub async fn compress(&self, session_id: &str) -> Result<CompressedSegment, CompressionError> {
        tracing::info!(session_id, "starting compression");
        // Get all messages
        let messages = self.store
            .get_messages(session_id, usize::MAX, 0)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        tracing::debug!(session_id, total_messages = messages.len(), "retrieved messages for compression");

        // Filter out already compressed messages (those with id where we can check)
        // For simplicity, we compress the first batch of messages
        let messages_to_compress = messages
            .into_iter()
            .take(self.config.min_compression_unit * 4) // Compress up to 4x min unit
            .collect::<Vec<_>>();

        if messages_to_compress.len() < self.config.min_compression_unit {
            tracing::warn!(session_id, messages_to_compress = messages_to_compress.len(), min_required = self.config.min_compression_unit, "not enough messages to compress");
            return Err(CompressionError::Config(
                "Not enough messages to compress".into()
            ));
        }

        // Group messages into segments (up to MAX_MESSAGES_PER_SEGMENT messages per segment)
        let segment_messages = &messages_to_compress[..messages_to_compress.len().min(MAX_MESSAGES_PER_SEGMENT)];
        tracing::debug!(session_id, segment_size = segment_messages.len(), "grouping messages into segment");

        // Generate summary
        let summary = self.summarizer
            .summarize(segment_messages, self.config.max_summary_tokens)
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        tracing::debug!(session_id, summary_len = summary.len(), "summary generated");

        // Generate embedding
        let vector = self.summarizer
            .embed(&summary)
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        // Create compressed segment
        let start_id = segment_messages.first()
            .map(|m| m.id)
            .ok_or_else(|| CompressionError::Config("No messages".into()))?;
        let end_id = segment_messages.last()
            .map(|m| m.id)
            .ok_or_else(|| CompressionError::Config("No messages".into()))?;

        let segment = CompressedSegment::new(
            session_id.to_string(),
            start_id,
            end_id,
            summary,
            vector,
        );

        // Store compressed segment
        self.store
            .insert_compressed_segment(&segment)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        // Mark original messages as compressed
        self.store
            .mark_messages_compressed(session_id, start_id, end_id)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))?;

        tracing::info!(session_id, start_id, end_id, "compression completed successfully");
        Ok(segment)
    }

    /// Get compressed segments for retrieval
    pub async fn get_compressed_segments(
        &self,
        session_id: &str,
    ) -> Result<Vec<CompressedSegment>, CompressionError> {
        self.store
            .get_compressed_segments(session_id)
            .await
            .map_err(|e| CompressionError::Storage(e.to_string()))
    }
}