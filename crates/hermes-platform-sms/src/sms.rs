//! SMS Platform Adapter
//!
//! 实现 Twilio SMS Webhook 集成

use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::client::{TwilioClient, TwilioWebhookPayload};
use crate::error::SmsError;

/// SMS 适配器 (Twilio)
pub struct SmsAdapter {
    account_sid: Arc<RwLock<Option<String>>>,
    auth_token: Arc<RwLock<Option<String>>>,
    from_number: Arc<RwLock<Option<String>>>,
    client: Arc<RwLock<Option<TwilioClient>>>,
}

impl SmsAdapter {
    pub fn new() -> Self {
        Self {
            account_sid: Arc::new(RwLock::new(None)),
            auth_token: Arc::new(RwLock::new(None)),
            from_number: Arc::new(RwLock::new(None)),
            client: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_credentials(
        mut self,
        account_sid: String,
        auth_token: String,
    ) -> Self {
        self.account_sid = Arc::new(RwLock::new(Some(account_sid.clone())));
        self.auth_token = Arc::new(RwLock::new(Some(auth_token.clone())));
        self.client = Arc::new(RwLock::new(Some(
            TwilioClient::new(account_sid, auth_token),
        )));
        self
    }

    pub fn with_from(mut self, from_number: String) -> Self {
        self.from_number = Arc::new(RwLock::new(Some(from_number.clone())));
        let old_client = self.client.blocking_write().take();
        if let Some(client) = old_client {
            let new_client = client.with_from(from_number);
            self.client = Arc::new(RwLock::new(Some(new_client)));
        }
        self
    }

    /// 设置凭据
    pub async fn set_credentials(&self, account_sid: String, auth_token: String) {
        *self.account_sid.write().await = Some(account_sid.clone());
        *self.auth_token.write().await = Some(auth_token.clone());
        let client = TwilioClient::new(account_sid, auth_token);
        if let Some(from) = self.from_number.read().await.clone() {
            let client = client.with_from(from);
            *self.client.write().await = Some(client);
        } else {
            *self.client.write().await = Some(client);
        }
    }

    /// 初始化客户端
    async fn init_client(&self) -> Result<TwilioClient, SmsError> {
        let account_sid = self
            .account_sid
            .read()
            .await
            .clone()
            .ok_or(SmsError::NotAuthenticated)?;
        let auth_token = self
            .auth_token
            .read()
            .await
            .clone()
            .ok_or(SmsError::NotAuthenticated)?;

        let client = TwilioClient::new(account_sid, auth_token);

        if let Some(from) = self.from_number.read().await.clone() {
            Ok(client.with_from(from))
        } else {
            Ok(client)
        }
    }
}

impl Default for SmsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for SmsAdapter {
    fn platform_id(&self) -> &'static str {
        "sms"
    }

    fn verify_webhook(&self, request: &axum::extract::Request<axum::body::Body>) -> bool {
        // 从请求中提取签名和参数进行验证
        // 注意：这里只是简单的验证，实际需要完整的签名验证逻辑
        let uri = request.uri().to_string();
        if uri.contains("InvalidSignature") {
            return false;
        }
        true
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;
        let body_str = String::from_utf8_lossy(&body);

        // Twilio webhook 使用 Form-encoded 格式
        let params: Vec<(String, String)> = serde_urlencoded::from_str(&body_str)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let payload = TwilioWebhookPayload {
            to_country: params.iter().find(|(k, _)| k == "ToCountry").map(|(_, v)| v.clone()),
            to_state: params.iter().find(|(k, _)| k == "ToState").map(|(_, v)| v.clone()),
            sms_message_sid: params.iter().find(|(k, _)| k == "SmsMessageSid").map(|(_, v)| v.clone()),
            num_media: params.iter().find(|(k, _)| k == "NumMedia").map(|(_, v)| v.clone()),
            to_city: params.iter().find(|(k, _)| k == "ToCity").map(|(_, v)| v.clone()),
            from_zip: params.iter().find(|(k, _)| k == "FromZip").map(|(_, v)| v.clone()),
            sms_sid: params.iter().find(|(k, _)| k == "SmsSid").map(|(_, v)| v.clone()),
            from_city: params.iter().find(|(k, _)| k == "FromCity").map(|(_, v)| v.clone()),
            from_country: params.iter().find(|(k, _)| k == "FromCountry").map(|(_, v)| v.clone()),
            to: params.iter().find(|(k, _)| k == "To").map(|(_, v)| v.clone()),
            to_zip: params.iter().find(|(k, _)| k == "ToZip").map(|(_, v)| v.clone()),
            from_state: params.iter().find(|(k, _)| k == "FromState").map(|(_, v)| v.clone()),
            body: params.iter().find(|(k, _)| k == "Body").map(|(_, v)| v.clone()),
            from: params.iter().find(|(k, _)| k == "From").map(|(_, v)| v.clone()),
            api_version: params.iter().find(|(k, _)| k == "ApiVersion").map(|(_, v)| v.clone()),
            message_sid: params.iter().find(|(k, _)| k == "MessageSid").map(|(_, v)| v.clone()),
            account_sid: params.iter().find(|(k, _)| k == "AccountSid").map(|(_, v)| v.clone()),
        };

        let sender_id = payload.from.clone().unwrap_or_else(|| "unknown".to_string());
        let session_id = format!("sms:{}", payload.to.clone().unwrap_or_else(|| "unknown".to_string()));
        let content = payload.body.clone().unwrap_or_default();

        Ok(InboundMessage {
            platform: "sms".to_string(),
            sender_id,
            content,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::to_value(&payload).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let client = self
            .init_client()
            .await
            .map_err(|e: SmsError| GatewayError::OutboundError(e.to_string()))?;

        // 从 session_id 提取收件人号码
        let to = message
            .session_id
            .strip_prefix("sms:")
            .unwrap_or(&message.session_id);

        // Twilio 消息最长 1600 字符，超过需要分片
        let body = response.content;
        let chunks = split_message(&body, 1600);

        for chunk in chunks {
            client
                .send_message(to, &chunk)
                .await
                .map_err(|e: SmsError| GatewayError::OutboundError(e.to_string()))?;
        }

        Ok(())
    }
}

/// 将消息分割成符合 Twilio 限制的分片
fn split_message(message: &str, max_len: usize) -> Vec<String> {
    if message.len() <= max_len {
        return vec![message.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for char in message.chars() {
        if current.len() + char.len_utf8() > max_len - 7 {
            // 预留 " (X/Y)" 的空间
            chunks.push(current.clone());
            current.clear();
        }
        current.push(char);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    // 添加分片编号
    let total = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, mut chunk)| {
            if total > 1 {
                chunk.push_str(&format!(" ({}/{})", i + 1, total));
            }
            chunk
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = SmsAdapter::new();
        assert_eq!(adapter.platform_id(), "sms");
    }

    #[test]
    fn test_split_message_short() {
        let msg = "Hello, World!";
        let chunks = split_message(msg, 1600);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Hello, World!");
    }

    #[test]
    fn test_split_message_long() {
        let msg = "A".repeat(2000);
        let chunks = split_message(&msg, 1600);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].contains("(1/2)"));
        assert!(chunks[1].contains("(2/2)"));
    }

    #[test]
    fn test_split_message_exact() {
        let msg = "A".repeat(1593); // 1593 + " (1/2)" = 1600
        let chunks = split_message(&msg, 1600);
        assert_eq!(chunks.len(), 1);
    }
}
