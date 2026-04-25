//! Compression error types

/// Compression operation errors
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("LLM API error: {0}")]
    LlmApi(String),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Storage error: {0}")]
    Storage(String),
}
