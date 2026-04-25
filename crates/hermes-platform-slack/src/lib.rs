//! Slack Platform Adapter
//!
//! 实现 Slack Bot 的 Webhook 集成 (Events API 和 Interactive Messages)

use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Slack 适配器
pub struct SlackAdapter {
    signing_secret: Arc<RwLock<Option<String>>>,
    bot_token: Arc<RwLock<Option<String>>>,
    team_id: Arc<RwLock<Option<String>>>,
}

impl SlackAdapter {
    pub fn new() -> Self {
        Self {
            signing_secret: Arc::new(RwLock::new(None)),
            bot_token: Arc::new(RwLock::new(None)),
            team_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_signing_secret(mut self, secret: String) -> Self {
        self.signing_secret = Arc::new(RwLock::new(Some(secret)));
        self
    }

    pub fn with_bot_token(mut self, token: String) -> Self {
        self.bot_token = Arc::new(RwLock::new(Some(token)));
        self
    }

    pub fn with_team_id(mut self, team_id: String) -> Self {
        self.team_id = Arc::new(RwLock::new(Some(team_id)));
        self
    }

    /// 设置 Signing Secret
    pub async fn set_signing_secret(&self, secret: String) {
        *self.signing_secret.write().await = Some(secret);
    }

    /// 设置 Bot Token
    pub async fn set_bot_token(&self, token: String) {
        *self.bot_token.write().await = Some(token);
    }
}

impl Default for SlackAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Slack Events API 事件封装
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackEventWrapper {
    pub token: Option<String>,
    pub team_id: Option<String>,
    pub api_app_id: Option<String>,
    pub event: Option<SlackEvent>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub challenge: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SlackEvent {
    #[serde(rename = "event_callback")]
    EventCallback {
        event: SlackEventData,
        authed_users: Option<Vec<String>>,
    },
    #[serde(rename = "url_verification")]
    UrlVerification { challenge: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SlackEventData {
    #[serde(rename = "message")]
    Message(SlackMessage),
    #[serde(rename = "app_mention")]
    AppMention(SlackMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessage {
    pub channel: String,
    pub user: String,
    pub text: String,
    pub ts: String,
    pub thread_ts: Option<String>,
}

/// Slack Interactive Message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackInteractionPayload {
    pub token: String,
    pub team: SlackTeamInfo,
    pub channel: SlackChannelInfo,
    pub user: SlackUserInfo,
    pub callback_id: Option<String>,
    pub trigger_id: Option<String>,
    #[serde(rename = "type")]
    pub interaction_type: String,
    pub message: Option<SlackMessage>,
    pub actions: Option<Vec<SlackAction>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackTeamInfo {
    pub id: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackChannelInfo {
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackUserInfo {
    pub id: String,
    pub name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAction {
    pub action_id: String,
    pub block_id: Option<String>,
    pub action_ts: String,
    #[serde(rename = "type")]
    pub action_type: String,
    pub text: Option<SlackActionText>,
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackActionText {
    pub text: String,
    pub r#type: String,
}

/// Slack 响应消息结构
#[derive(Debug, Serialize)]
pub struct SlackResponse {
    pub text: Option<String>,
    pub blocks: Option<Vec<SlackBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
    pub replace_original: Option<bool>,
    pub delete_original: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SlackBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<SlackBlockText>,
    pub elements: Option<Vec<SlackBlockElement>>,
}

#[derive(Debug, Serialize)]
pub struct SlackBlockText {
    #[serde(rename = "type")]
    pub text_type: String,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct SlackBlockElement {
    #[serde(rename = "type")]
    pub element_type: String,
    pub text: Option<SlackBlockText>,
    pub action_id: Option<String>,
}

impl SlackResponse {
    pub fn new(text: &str) -> Self {
        Self {
            text: Some(text.to_string()),
            blocks: None,
            thread_ts: None,
            replace_original: None,
            delete_original: None,
        }
    }

    pub fn in_thread(mut self, thread_ts: &str) -> Self {
        self.thread_ts = Some(thread_ts.to_string());
        self
    }

    pub fn as_ephemeral(user_id: &str) -> Self {
        Self {
            text: None,
            blocks: None,
            thread_ts: None,
            replace_original: None,
            delete_original: None,
        }
    }
}

#[async_trait]
impl PlatformAdapter for SlackAdapter {
    fn platform_id(&self) -> &'static str {
        "slack"
    }

    fn verify_webhook(&self, _request: &axum::extract::Request<axum::body::Body>) -> bool {
        // TODO: 实现 Slack signing secret 验证
        // Slack 使用 HMAC-SHA256 签名验证请求
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

        // 尝试解析为 URL verification challenge
        let verify: SlackEventWrapper = serde_json::from_str(&body_str)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        // URL 验证请求
        if verify.event_type.as_deref() == Some("url_verification") {
            let challenge = verify.challenge.clone().unwrap_or_default();
            return Ok(InboundMessage {
                platform: "slack".to_string(),
                sender_id: "slack".to_string(),
                content: challenge,
                session_id: "slack:url_verification".to_string(),
                timestamp: Utc::now(),
                raw: serde_json::to_value(&verify).unwrap_or_default(),
            });
        }

        // 处理事件回调
        if let Some(event) = verify.event {
            match event {
                SlackEvent::EventCallback { event: event_data, .. } => {
                    match event_data {
                        SlackEventData::Message(msg) => {
                            let sender_id = msg.user.clone();
                            let session_id = format!("slack:{}", msg.channel);
                            let text = msg.text.clone();
                            return Ok(InboundMessage {
                                platform: "slack".to_string(),
                                sender_id,
                                content: text,
                                session_id,
                                timestamp: Utc::now(),
                                raw: serde_json::to_value(&msg).unwrap_or_default(),
                            });
                        }
                        SlackEventData::AppMention(msg) => {
                            let sender_id = msg.user.clone();
                            let session_id = format!("slack:{}", msg.channel);
                            let text = msg.text.clone();
                            return Ok(InboundMessage {
                                platform: "slack".to_string(),
                                sender_id,
                                content: text,
                                session_id,
                                timestamp: Utc::now(),
                                raw: serde_json::to_value(&msg).unwrap_or_default(),
                            });
                        }
                    }
                }
                SlackEvent::UrlVerification { challenge } => {
                    return Ok(InboundMessage {
                        platform: "slack".to_string(),
                        sender_id: "slack".to_string(),
                        content: challenge,
                        session_id: "slack:verification".to_string(),
                        timestamp: Utc::now(),
                        raw: serde_json::json!({"type": "url_verification"}),
                    });
                }
            }
        }

        // 尝试解析为 Interactive Message payload
        let interaction: Result<SlackInteractionPayload, _> = serde_json::from_str(&body_str);
        if let Ok(payload) = interaction {
            let sender_id = payload.user.id.clone();
            let session_id = format!("slack:{}", payload.channel.id);

            let content = if let Some(actions) = &payload.actions {
                actions.iter()
                    .filter_map(|a| a.value.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                payload.callback_id.clone().unwrap_or_default()
            };

            return Ok(InboundMessage {
                platform: "slack".to_string(),
                sender_id,
                content,
                session_id,
                timestamp: Utc::now(),
                raw: serde_json::to_value(&payload).unwrap_or_default(),
            });
        }

        Err(GatewayError::ParseError("Unknown Slack payload format".to_string()))
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let slack_response = SlackResponse::new(&response.content);

        tracing::info!("Slack outbound: {:?}", slack_response);
        // TODO: 实现实际发送逻辑 via Slack API
        let _ = message;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = SlackAdapter::new();
        assert_eq!(adapter.platform_id(), "slack");
    }

    #[test]
    fn test_slack_response_new() {
        let response = SlackResponse::new("Hello, Slack!");
        assert_eq!(response.text, Some("Hello, Slack!".to_string()));
    }

    #[test]
    fn test_slack_response_in_thread() {
        let response = SlackResponse::new("Reply").in_thread("12345.67890");
        assert_eq!(response.thread_ts, Some("12345.67890".to_string()));
    }
}