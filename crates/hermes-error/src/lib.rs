use thiserror::Error;

/// Shared storage error — kept in this crate so hermes-memory can use it
/// without creating a cyclic dependency through hermes-core.
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Query failed: {0}")]
    Query(String),

    #[error("Migration failed: {0}")]
    Migration(String),

    #[error("Busy, try again")]
    Busy,

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}
