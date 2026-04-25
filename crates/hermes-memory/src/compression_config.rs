//! Compression configuration

use serde::{Deserialize, Serialize};

/// Compression mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    SummaryOnly,
    VectorOnly,
    Hybrid,
}

impl Default for CompressionMode {
    fn default() -> Self {
        Self::Hybrid
    }
}

/// LLM provider type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SummarizerProvider {
    OpenAi,
    Ollama,
}

impl Default for SummarizerProvider {
    fn default() -> Self {
        Self::OpenAi
    }
}

/// Context compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable context compression
    pub enabled: bool,
    /// Token count threshold to trigger compression
    pub token_threshold: usize,
    /// Message count threshold to trigger compression
    pub message_count_threshold: usize,
    /// Minimum number of messages to compress at once
    pub min_compression_unit: usize,
    /// Maximum summary length in tokens
    pub max_summary_tokens: usize,
    /// Compression mode
    pub mode: CompressionMode,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            token_threshold: 8000,
            message_count_threshold: 50,
            min_compression_unit: 5,
            max_summary_tokens: 500,
            mode: CompressionMode::Hybrid,
        }
    }
}

/// Summarizer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizerConfig {
    /// Provider type
    pub provider: SummarizerProvider,
    /// Model name
    pub model: String,
    /// Ollama URL (for local models)
    pub ollama_url: Option<String>,
}

impl Default for SummarizerConfig {
    fn default() -> Self {
        Self {
            provider: SummarizerProvider::OpenAi,
            model: "gpt-4o-mini".to_string(),
            ollama_url: Some("http://localhost:11434".to_string()),
        }
    }
}