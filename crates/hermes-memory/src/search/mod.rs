//! Session search with FTS5 + LLM summarization

pub mod fts;
pub mod summarizer;

pub use fts::sanitize_fts_query;
pub use summarizer::SessionSummarizer;