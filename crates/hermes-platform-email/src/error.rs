//! Email error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("SMTP error: {0}")]
    Smtp(String),
    
    #[error("IMAP error: {0}")]
    Imap(String),
    
    #[error("Webhook verification failed")]
    VerificationFailed,
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
}
