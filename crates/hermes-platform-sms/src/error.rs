//! SMS (Twilio) Error Types

/// SMS 错误类型
#[derive(Debug, thiserror::Error)]
pub enum SmsError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Signature verification failed")]
    InvalidSignature,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Send message error: {0}")]
    SendMessage(String),
}
