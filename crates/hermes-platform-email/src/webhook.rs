//! Webhook handlers for email providers (SendGrid, Mailgun, SES)

use serde::{Deserialize, Serialize};

/// Webhook configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebhookConfig {
    pub provider: WebhookProvider,
    pub secret: Option<String>,
}

/// Supported webhook providers
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WebhookProvider {
    SendGrid,
    Mailgun,
    Ses,
}
