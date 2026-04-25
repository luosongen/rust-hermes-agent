//! Email parsing utilities

use crate::error::EmailError;

/// Email message structure
#[derive(Debug, Clone)]
pub struct Email {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
}

/// Parse email message from raw bytes
pub fn parse_email(raw: &[u8]) -> Result<mail_parser::Message<'_>, EmailError> {
    mail_parser::Message::parse(raw)
        .ok_or_else(|| EmailError::ParseError("Failed to parse email".into()))
}

/// Extract address string from HeaderValue
fn address_to_string(h: &mail_parser::HeaderValue) -> String {
    match h {
        mail_parser::HeaderValue::Address(addr) => {
            addr.address.as_ref().map(|s| s.to_string()).unwrap_or_default()
        }
        mail_parser::HeaderValue::Group(group) => {
            group.addresses.first()
                .and_then(|a| a.address.as_ref())
                .map(|s| s.to_string())
                .unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// Parse email into our Email struct with proper field mapping
/// Spec mapping: From→sender_id, To→session_id as `email:<address>`, Subject→raw, Body→content
pub fn parse_email_to_inbound(raw: &[u8]) -> Result<Email, EmailError> {
    let msg = parse_email(raw)?;

    // Extract sender (From)
    let from = address_to_string(msg.get_from());

    // Extract recipients (To)
    let to = address_to_string(msg.get_to());
    let to_vec = if to.is_empty() {
        vec![]
    } else {
        vec![to]
    };

    // Extract subject
    let subject = msg
        .get_subject()
        .map(|s| s.to_string())
        .unwrap_or_default();

    // Extract body - use get_text_body with position 0
    let body = msg
        .get_text_body(0)
        .map(|t| t.to_string())
        .unwrap_or_default();

    Ok(Email {
        from,
        to: to_vec,
        subject,
        body,
    })
}
