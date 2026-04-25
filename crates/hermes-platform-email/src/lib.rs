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
pub use webhook::{WebhookConfig, WebhookProvider, verify_webhook_signature};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde_json::Value;
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
    /// 创建新的 EmailAdapter 实例
    pub fn new() -> Self {
        Self {
            smtp_config: Arc::new(RwLock::new(None)),
            imap_config: Arc::new(RwLock::new(None)),
            webhook_config: Arc::new(RwLock::new(None)),
            smtp_client: Arc::new(RwLock::new(None)),
        }
    }

    /// 配置 SMTP
    pub fn with_smtp(mut self, config: SmtpConfig) -> Self {
        self.smtp_config = Arc::new(RwLock::new(Some(config.clone())));
        self.smtp_client = Arc::new(RwLock::new(Some(SmtpClient::new(config))));
        self
    }

    /// 配置 IMAP
    pub fn with_imap(mut self, config: ImapConfig) -> Self {
        self.imap_config = Arc::new(RwLock::new(Some(config)));
        self
    }

    /// 配置 Webhook
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

    fn verify_webhook(&self, request: &Request<Body>) -> bool {
        // Get webhook config
        let webhook_config = self.webhook_config.clone();
        let config_guard = tokio::runtime::Handle::current().block_on(async {
            webhook_config.read().await
        });

        let config = match config_guard.as_ref() {
            Some(cfg) => cfg,
            None => return false,
        };

        // For each provider, verify the signature
        for provider in &config.providers {
            if verify_webhook_signature(request, provider, &config.secret).is_ok() {
                return true;
            }
        }

        false
    }

    async fn parse_inbound(
        &self,
        request: Request<Body>,
    ) -> Result<InboundMessage, GatewayError> {
        // Get the full body
        let body = axum::body::to_bytes(request.into_body(), 10 * 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(format!("Failed to read body: {}", e)))?;

        // Parse email
        let email = crate::parser::parse_email_to_inbound(&body)
            .map_err(|e| GatewayError::ParseError(format!("Failed to parse email: {}", e)))?;

        // Map to InboundMessage according to spec:
        // From → sender_id
        // To → session_id as `email:<address>`
        // Subject → raw
        // Body → content
        let sender_id = email.from.clone();
        let session_id = format!("email:{}", email.to.first().unwrap_or(&email.from));
        let raw = Value::String(email.subject.clone());
        let content = email.body.clone();

        Ok(InboundMessage {
            platform: "email".to_string(),
            sender_id,
            session_id,
            raw,
            content,
            timestamp: chrono::Utc::now(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let smtp_client_guard = self.smtp_client.read().await;
        let client = smtp_client_guard
            .as_ref()
            .ok_or_else(|| GatewayError::OutboundError("SMTP client not configured".into()))?;

        // Extract the from address from the session_id (email:address format)
        let to = message.session_id.replace("email:", "");

        // Use the response content or format it
        let body = response.content.clone();
        let subject = format!("Re: {}", message.raw.as_str().unwrap_or_default());

        client
            .send(&to, &subject, &body)
            .await
            .map_err(|e| GatewayError::OutboundError(format!("Failed to send email: {}", e)))?;

        Ok(())
    }
}
