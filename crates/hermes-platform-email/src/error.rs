//! Email error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("SMTP connection error: {0}")]
    SmtpConnection(String),

    #[error("SMTP authentication error: {0}")]
    SmtpAuth(String),

    #[error("IMAP connection error: {0}")]
    ImapConnection(String),

    #[error("IMAP authentication error: {0}")]
    ImapAuth(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Webhook signature verification failed")]
    WebhookVerificationFailed,

    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Send error: {0}")]
    Send(String),
}
