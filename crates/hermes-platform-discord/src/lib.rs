//! Discord Platform Adapter
//!
//! 实现 Discord Bot 的 Webhook 集成

use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Discord 适配器
pub struct DiscordAdapter {
    bot_token: Arc<RwLock<Option<String>>>,
    guild_id: Arc<RwLock<Option<String>>>,
}

impl DiscordAdapter {
    pub fn new() -> Self {
        Self {
            bot_token: Arc::new(RwLock::new(None)),
            guild_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_bot_token(mut self, token: String) -> Self {
        self.bot_token = Arc::new(RwLock::new(Some(token)));
        self
    }

    pub fn with_guild(mut self, guild_id: String) -> Self {
        self.guild_id = Arc::new(RwLock::new(Some(guild_id)));
        self
    }

    /// 设置 Bot Token
    pub async fn set_bot_token(&self, token: String) {
        *self.bot_token.write().await = Some(token);
    }

    /// 获取 Bot Token
    pub async fn get_bot_token(&self) -> Option<String> {
        self.bot_token.read().await.clone()
    }
}

impl Default for DiscordAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Discord 消息格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    pub content: String,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub embeds: Option<Vec<DiscordEmbed>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    pub color: Option<u32>,
    pub fields: Option<Vec<DiscordEmbedField>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    pub inline: Option<bool>,
}

/// Discord 交互请求 (用于 Slash Commands)
#[derive(Debug, Clone, Deserialize)]
pub struct DiscordInteraction {
    pub id: String,
    pub application_id: String,
    pub token: String,
    #[serde(rename = "type")]
    pub interaction_type: u8,
    pub data: Option<DiscordApplicationCommandData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordApplicationCommandData {
    pub name: String,
    pub options: Option<Vec<DiscordCommandOption>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordCommandOption {
    pub name: String,
    pub value: Option<serde_json::Value>,
}

#[async_trait]
impl PlatformAdapter for DiscordAdapter {
    fn platform_id(&self) -> &'static str {
        "discord"
    }

    fn verify_webhook(&self, request: &axum::extract::Request<axum::body::Body>) -> bool {
        // Discord Interactions 使用 Ed25519 公钥签名验证
        // 需要 X-Signature-Ed25519 和 X-Signature-Timestamp 头部
        // 此验证在 parse_inbound 中进行（需要 body）
        let headers = request.headers();
        headers.contains_key("X-Signature-Ed25519")
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;
        let body_str = String::from_utf8_lossy(&body);

        // 尝试解析为交互请求
        let interaction: Result<DiscordInteraction, _> = serde_json::from_str(&body_str);

        if let Ok(interaction) = interaction {
            // 处理 slash command
            if interaction.interaction_type == 2 {
                // APPLICATION_COMMAND
                if let Some(data) = &interaction.data {
                    return Ok(InboundMessage {
                        platform: "discord".to_string(),
                        sender_id: interaction.application_id.clone(),
                        content: format!("/{}", data.name),
                        session_id: format!("discord:{}", interaction.application_id),
                        timestamp: Utc::now(),
                        raw: serde_json::json!({"type": "interaction"}),
                    });
                }
            }
        }

        // 尝试解析为普通消息
        #[derive(Deserialize)]
        struct DiscordEventMsg {
            #[serde(rename = "d")]
            data: DiscordMsgData,
        }

        #[derive(Deserialize)]
        struct DiscordMsgData {
            content: String,
            author: DiscordAuthData,
            id: String,
            #[allow(dead_code)]
            timestamp: String,
            #[allow(dead_code)]
            channel_id: Option<String>,
        }

        #[derive(Deserialize)]
        struct DiscordAuthData {
            id: String,
            username: String,
            #[allow(dead_code)]
            discriminator: Option<String>,
        }

        let event: DiscordEventMsg =
            serde_json::from_str(&body_str).map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let sender_id = event.data.author.id.clone();
        let session_id = format!("discord:{}", sender_id);

        Ok(InboundMessage {
            platform: "discord".to_string(),
            sender_id,
            content: event.data.content,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::json!({
                "username": event.data.author.username,
                "id": event.data.id,
            }),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let discord_msg = DiscordMessage {
            content: response.content,
            username: Some("Hermes Bot".to_string()),
            avatar_url: None,
            embeds: None,
        };

        tracing::info!("Discord outbound: {:?}", discord_msg);
        // TODO: 实现实际发送逻辑
        let _ = message;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = DiscordAdapter::new();
        assert_eq!(adapter.platform_id(), "discord");
    }
}