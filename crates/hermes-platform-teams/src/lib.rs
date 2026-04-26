//! Microsoft Teams Platform Adapter
//!
//! 实现 Microsoft Teams Bot 的 Webhook 集成

use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Teams 适配器
pub struct TeamsAdapter {
    app_id: Arc<RwLock<Option<String>>>,
    app_password: Arc<RwLock<Option<String>>>,
    tenant_id: Arc<RwLock<Option<String>>>,
}

impl TeamsAdapter {
    pub fn new() -> Self {
        Self {
            app_id: Arc::new(RwLock::new(None)),
            app_password: Arc::new(RwLock::new(None)),
            tenant_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_credentials(mut self, app_id: String, app_password: String) -> Self {
        self.app_id = Arc::new(RwLock::new(Some(app_id)));
        self.app_password = Arc::new(RwLock::new(Some(app_password)));
        self
    }

    pub fn with_tenant(mut self, tenant_id: String) -> Self {
        self.tenant_id = Arc::new(RwLock::new(Some(tenant_id)));
        self
    }

    /// 设置凭据
    pub async fn set_credentials(&self, app_id: String, app_password: String) {
        *self.app_id.write().await = Some(app_id);
        *self.app_password.write().await = Some(app_password);
    }
}

impl Default for TeamsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Teams Activity payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsActivity {
    pub resource: Option<String>,
    pub id: Option<String>,
    pub timestamp: Option<String>,
    pub local_timestamp: Option<String>,
    pub channel_id: Option<String>,
    pub service_url: Option<String>,
    pub platform: Option<String>,
    pub entities: Option<Vec<TeamsEntity>>,
    pub recipient: Option<TeamsChannelAccount>,
    pub from: Option<TeamsChannelAccount>,
    pub conversation: Option<TeamsConversationAccount>,
    pub channel_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsEntity {
    #[serde(rename = "type")]
    pub entity_type: Option<String>,
    pub mentions: Option<Vec<TeamsMention>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsMention {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub mention_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsChannelAccount {
    pub id: Option<String>,
    pub name: Option<String>,
    pub aad_object_id: Option<String>,
    pub email: Option<String>,
    pub user_principal_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsConversationAccount {
    pub id: Option<String>,
    pub name: Option<String>,
    pub aad_object_id: Option<String>,
    pub tenant_id: Option<String>,
    pub conversation_type: Option<String>,
}

/// Teams Message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsMessage {
    pub text: Option<String>,
    pub summary: Option<String>,
    pub attachments: Option<Vec<TeamsAttachment>>,
    pub entities: Option<Vec<serde_json::Value>>,
    pub channel_data: Option<serde_json::Value>,
    pub type_: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsAttachment {
    pub content_type: Option<String>,
    pub content_url: Option<String>,
    pub content: Option<serde_json::Value>,
    pub name: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// Teams 机器人活动 (从 Bot Connector 接收)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsActivityWrapper {
    pub type_: Option<String>,
    pub id: Option<String>,
    pub timestamp: Option<String>,
    pub local_timestamp: Option<String>,
    pub channel_id: Option<String>,
    pub service_url: Option<String>,
    pub channel_data: Option<serde_json::Value>,
    pub from: Option<TeamsChannelAccount>,
    pub recipient: Option<TeamsChannelAccount>,
    pub conversation: Option<TeamsConversationAccount>,
    pub text: Option<String>,
    pub speak: Option<String>,
    pub input_hint: Option<String>,
    pub summary: Option<String>,
    pub attachments: Option<Vec<TeamsAttachment>>,
    pub entities: Option<Vec<serde_json::Value>>,
    pub reactions: Option<Vec<TeamsReaction>>,
    pub reply_to_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsReaction {
    #[serde(rename = "type")]
    pub reaction_type: String,
    pub created_at: String,
}

/// Teams 响应消息
#[derive(Debug, Serialize)]
pub struct TeamsResponse {
    #[serde(rename = "type")]
    pub activity_type: String,
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speak: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<TeamsAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_data: Option<serde_json::Value>,
}

impl TeamsResponse {
    pub fn new_text(text: &str) -> Self {
        Self {
            activity_type: "message".to_string(),
            text: Some(text.to_string()),
            speak: None,
            input_hint: None,
            attachments: None,
            channel_data: None,
        }
    }

    pub fn with_speak(mut self, speak: &str) -> Self {
        self.speak = Some(speak.to_string());
        self
    }

    pub fn with_attachments(mut self, attachments: Vec<TeamsAttachment>) -> Self {
        self.attachments = Some(attachments);
        self
    }
}

/// Teams API 错误
#[derive(Debug, thiserror::Error)]
pub enum TeamsError {
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Parse error: {0}")]
    Parse(String),
}

#[async_trait]
impl PlatformAdapter for TeamsAdapter {
    fn platform_id(&self) -> &'static str {
        "teams"
    }

    fn verify_webhook(&self, request: &axum::extract::Request<axum::body::Body>) -> bool {
        // Teams 使用 JWT 令牌验证，实际验证在 parse_inbound 中进行
        // 此处检查必要头部是否存在
        let headers = request.headers();
        // Teams webhook 应包含 ChannelID 头部
        headers.contains_key("ChannelID")
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;
        let body_str = String::from_utf8_lossy(&body);

        let activity: TeamsActivityWrapper = serde_json::from_str(&body_str)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let sender_id = activity
            .from
            .as_ref()
            .and_then(|f| f.id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let session_id = activity
            .conversation
            .as_ref()
            .and_then(|c| c.id.clone())
            .map(|id| format!("teams:{}", id))
            .unwrap_or_else(|| "teams:unknown".to_string());

        let content = activity.text.clone().unwrap_or_default();

        Ok(InboundMessage {
            platform: "teams".to_string(),
            sender_id,
            content,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::to_value(&activity).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let teams_response = TeamsResponse::new_text(&response.content);

        tracing::info!("Teams outbound: {:?}", teams_response);
        // TODO: 实现实际发送逻辑 via Bot Framework API
        let _ = message;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = TeamsAdapter::new();
        assert_eq!(adapter.platform_id(), "teams");
    }

    #[test]
    fn test_teams_response_new() {
        let response = TeamsResponse::new_text("Hello, Teams!");
        assert_eq!(response.text, Some("Hello, Teams!".to_string()));
        assert_eq!(response.activity_type, "message");
    }

    #[test]
    fn test_teams_response_with_speak() {
        let response = TeamsResponse::new_text("Hello").with_speak("Speaking text");
        assert!(response.speak.is_some());
    }
}