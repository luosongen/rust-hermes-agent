//! Email Platform Adapter
//!
//! 支持：
//! - 入站：Webhook（SendGrid/Mailgun/SES）+ IMAP 轮询
//! - 出站：SMTP 发送

pub mod error;
pub mod imap;
pub mod parser;
pub mod smtp;
pub mod webhook;

pub use error::EmailError;
pub use smtp::{SmtpClient, SmtpConfig};
pub use imap::{ImapConfig, ImapPoller};
pub use webhook::{WebhookConfig, WebhookProvider};

use async_trait::async_trait;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Email 适配器
pub struct EmailAdapter {
    smtp_config: Arc<RwLock<Option<SmtpConfig>>>,
    imap_config: Arc<RwLock<Option<ImapConfig>>>,
    webhook_config: Arc<RwLock<Option<WebhookConfig>>>,
    smtp_client: Arc<RwLock<Option<SmtpClient>>>,
}

impl EmailAdapter {
    pub fn new() -> Self {
        Self {
            smtp_config: Arc::new(RwLock::new(None)),
            imap_config: Arc::new(RwLock::new(None)),
            webhook_config: Arc::new(RwLock::new(None)),
            smtp_client: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_smtp(mut self, config: SmtpConfig) -> Self {
        self.smtp_config = Arc::new(RwLock::new(Some(config.clone())));
        self.smtp_client = Arc::new(RwLock::new(Some(SmtpClient::new(config))));
        self
    }

    pub fn with_imap(mut self, config: ImapConfig) -> Self {
        self.imap_config = Arc::new(RwLock::new(Some(config)));
        self
    }

    pub fn with_webhook(mut self, config: WebhookConfig) -> Self {
        self.webhook_config = Arc::new(RwLock::new(Some(config)));
        self
    }
}

impl Default for EmailAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for EmailAdapter {
    fn platform_id(&self) -> &'static str {
        "email"
    }

    fn verify_webhook(&self, _request: &axum::extract::Request<axum::body::Body>) -> bool {
        true
    }

    async fn parse_inbound(
        &self,
        _request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        Err(GatewayError::ParseError("Not implemented".into()))
    }

    async fn send_response(
        &self,
        _response: ConversationResponse,
        _message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        Err(GatewayError::OutboundError("Not implemented".into()))
    }
}
