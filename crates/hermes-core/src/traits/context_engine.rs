//! ContextEngine trait — pluggable context management strategies.

use crate::{Message, ToolError};
use async_trait::async_trait;

/// Compression status for monitoring/debugging.
#[derive(Debug, Clone)]
pub struct CompressionStatus {
    pub compression_count: usize,
    pub current_tokens: usize,
    pub threshold_tokens: usize,
    pub model: String,
}

/// Context management engine trait.
/// Implementations can be compressors (default), replacers, or other strategies.
#[async_trait]
pub trait ContextEngine {
    /// Returns the engine's name (e.g., "compressor", "dummy").
    fn name(&self) -> &str;

    /// Returns true when the prompt tokens exceed the compression threshold.
    fn should_compress(&self, prompt_tokens: usize) -> bool;

    /// Compress a message list, returning the compressed version.
    async fn compress(
        &self,
        messages: &[Message],
        prompt_tokens: usize,
        focus_topic: Option<&str>,
    ) -> Result<Vec<Message>, ToolError>;

    /// Reset engine state on new session.
    fn on_session_reset(&mut self);

    /// Current compression status for monitoring.
    fn get_status(&self) -> CompressionStatus;
}
