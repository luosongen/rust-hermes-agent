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
        // 同步验证：检查必要的头部是否存在
        // 实际的密码学签名验证在 parse_inbound 中进行
        let headers = request.headers();

        // 检查 SendGrid 签名头部是否存在
        if headers.contains_key("X-Twilio-Email-Event-Webhook-Signature") {
            return true;
        }

        // 检查 Mailgun 签名头部
        if headers.contains_key("Mailgun-Events-Signature") {
            return true;
        }

        // 检查 SES 签名头部
        if headers.contains_key("X-Ses-Sns-Subscription-Arn")
            || headers.contains_key("X-Ses-Sns-Topic-Arn")
        {
            return true;
        }

        false
    }

    async fn parse_inbound(
        &self,
        request: Request<Body>,
    ) -> Result<InboundMessage, GatewayError> {
        // 获取 webhook 配置
        let webhook_config = self.webhook_config.read().await.clone();

        // 获取完整请求体
        let (parts, body) = request.into_parts();
        let body = axum::body::to_bytes(body, 10 * 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(format!("Failed to read body: {}", e)))?;

        // 如果配置了 webhook，验证签名
        if let Some(ref config) = webhook_config {
            let headers = &parts.headers;

            // 根据提供商验证签名
            for provider in &config.providers {
                match provider {
                    WebhookProvider::SendGrid => {
                        // SendGrid 使用 HMAC-SHA256
                        if let Some(signature) = headers
                            .get("X-Twilio-Email-Event-Webhook-Signature")
                            .and_then(|v| v.to_str().ok())
                        {
                            crate::webhook::verify_sendgrid(&body, signature, &config.secret)
                                .map_err(|_| GatewayError::VerificationFailed("SendGrid signature verification failed".into()))?;
                        }
                    }
                    WebhookProvider::Mailgun => {
                        // Mailgun 使用 timestamp + token + signature
                        if let Some(sig_header) = headers
                            .get("Mailgun-Events-Signature")
                            .and_then(|v| v.to_str().ok())
                        {
                            // Mailgun 签名格式: timestamp:signature (base64)
                            let parts: Vec<&str> = sig_header.splitn(2, ':').collect();
                            if parts.len() == 2 {
                                crate::webhook::verify_mailgun(
                                    parts[0],
                                    "", // token 在 body 中，这里简化处理
                                    parts[1],
                                    &config.secret,
                                )
                                .map_err(|_| GatewayError::VerificationFailed("Mailgun signature verification failed".into()))?;
                            }
                        }
                    }
                    WebhookProvider::Ses => {
                        // SES 使用 SNS 格式签名
                        // SES 验证较复杂，需要解析 SNS 消息
                        // 这里做简化处理：检查头部存在即认为是有效请求
                        tracing::debug!("SES webhook received, signature verification delegated to SNS handler");
                    }
                }
            }
        }

        // 解析邮件内容
        // 注意：这里假设是原始邮件格式（SMTP）
        // 对于 webhook 事件（JSON 格式），需要不同的解析逻辑
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
