use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Webhook verification failed: {0}")]
    VerificationFailed(String),

    #[error("Failed to parse inbound message: {0}")]
    ParseError(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Outbound error: {0}")]
    OutboundError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}
