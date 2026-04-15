//! ## hermes-platform-telegram
//!
//! Telegram 平台适配器，将 Telegram Bot Webhook 集成到 Hermes 网关。
//!
//! ### 功能概述
//! - **Webhook 验证**：通过 URL 查询参数中的 `secret_token` 与预设值比对
//! - **入站解析**：将 Telegram Update JSON 解析为 `InboundMessage`
//! - **出站发送**：通过 Telegram Bot API `sendMessage` 接口回复用户
//!
//! ### 配置要求
//! 创建适配器时需提供：
//! - `bot_token`：Telegram Bot Token（从 @BotFather 获取）
//! - `verify_token`：自定义的 Webhook 验证密钥
//!
//! ### 消息格式
//! - 会话 ID 格式：`telegram:{chat_id}`
//! - 支持 Markdown 解析模式

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub struct TelegramAdapter {
    bot_token: String,
    verify_token: String,
    http: Client,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramUpdate {
    update_id: u64,
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramMessage {
    chat: TelegramChat,
    text: Option<String>,
    #[serde(default)]
    date: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct TelegramChat {
    id: i64,
}

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    chat_id: i64,
    text: String,
    #[serde(rename = "parse_mode")]
    parse_mode: Option<String>,
}

impl TelegramAdapter {
    pub fn new(bot_token: String, verify_token: String) -> Self {
        Self {
            bot_token,
            verify_token,
            http: Client::new(),
        }
    }
}

#[async_trait]
impl PlatformAdapter for TelegramAdapter {
    fn platform_id(&self) -> &str {
        "telegram"
    }

    fn verify_webhook(&self, request: &Request<Body>) -> bool {
        let query = request.uri().query().unwrap_or("");
        let token = query
            .split('&')
            .find(|s| s.starts_with("secret_token="))
            .and_then(|s| s.strip_prefix("secret_token=").map(|s| s.to_string()));
        token.map_or(false, |t| t == self.verify_token)
    }

    async fn parse_inbound(
        &self,
        request: Request<Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;
        let body_str = String::from_utf8_lossy(&body);

        let mut update: TelegramUpdate =
            serde_json::from_str(&body_str)
                .map_err(|e| GatewayError::ParseError(format!("telegram parse error: {}", e)))?;

        let message = update.message.take().ok_or_else(|| {
            GatewayError::ParseError("No message in Telegram update".into())
        })?;

        let text = message.text.unwrap_or_default();
        let sender_id = message.chat.id.to_string();
        let session_id = format!("telegram:{}", sender_id);

        let timestamp = chrono::DateTime::from_timestamp(message.date as i64, 0)
            .unwrap_or_else(Utc::now);

        Ok(InboundMessage {
            platform: "telegram".into(),
            sender_id,
            content: text,
            session_id,
            timestamp,
            raw: serde_json::to_value(&update).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let chat_id: i64 = message
            .sender_id
            .parse()
            .map_err(|e| GatewayError::OutboundError(format!("invalid chat_id: {}", e)))?;

        let body = SendMessageRequest {
            chat_id,
            text: response.content,
            parse_mode: Some("Markdown".into()),
        };

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        self.http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::OutboundError(e.to_string()))?;

        Ok(())
    }
}
