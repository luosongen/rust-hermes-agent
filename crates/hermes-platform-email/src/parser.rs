//! Email parsing utilities

use crate::error::EmailError;

/// Parse email message
pub fn parse_email(raw: &[u8]) -> Result<mail_parser::Message, EmailError> {
    mail_parser::Message::parse(raw)
        .ok_or_else(|| EmailError::Parse("Failed to parse email".into()))
}
