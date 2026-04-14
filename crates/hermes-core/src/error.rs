use thiserror::Error;
pub use hermes_error::StorageError;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Session error: {0}")]
    Session(#[from] SessionError),
    #[error("Platform error: {0}")]
    Platform(#[from] PlatformError),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Iteration budget exhausted")]
    IterationExhausted,
    #[error("Content was filtered")]
    ContentFiltered,
    #[error("Unknown finish reason")]
    UnknownFinishReason,
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Billing error: {0}")]
    BillingError(String),
}

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("API error: {0}")]
    Api(String),
    #[error("Authentication failed")]
    Auth,
    #[error("Rate limited, retry after {0}s")]
    RateLimit(u64),
    #[error("Context length exceeded")]
    ContextTooLarge,
    #[error("Invalid model: {0}")]
    InvalidModel(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

impl ProviderError {
    /// Returns true if this error is transient and worth retrying.
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::RateLimit(_) => true,
            ProviderError::Api(_) => true,
            ProviderError::Network(_) => true,
            ProviderError::Auth
            | ProviderError::InvalidModel(_)
            | ProviderError::ContextTooLarge => false,
        }
    }

    /// Returns the suggested retry-after seconds, if known.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            ProviderError::RateLimit(s) => Some(*s),
            _ => None,
        }
    }
}

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Execution failed: {0}")]
    Execution(String),
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("Missing required environment: {0}")]
    MissingEnv(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Timeout: {0}")]
    Timeout(String),
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Session expired")]
    Expired,
    #[error("Session corrupted: {0}")]
    Corrupted(String),
}

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Authentication failed")]
    Auth,
    #[error("Rate limited")]
    RateLimit,
    #[error("Invalid message format: {0}")]
    InvalidFormat(String),
    #[error("Webhook verification failed")]
    WebhookVerificationFailed,
    #[error("Message not found: {0}")]
    MessageNotFound(String),
}
