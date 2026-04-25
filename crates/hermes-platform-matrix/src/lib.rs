//! Matrix Platform Adapter
//!
//! 实现 Matrix Client-Server API 的 Webhook 集成

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Matrix 适配器
pub struct MatrixAdapter {
    homeserver_url: Arc<RwLock<Option<String>>>,
    access_token: Arc<RwLock<Option<String>>>,
    user_id: Arc<RwLock<Option<String>>>,
    room_id: Arc<RwLock<Option<String>>>,
}

impl MatrixAdapter {
    pub fn new() -> Self {
        Self {
            homeserver_url: Arc::new(RwLock::new(None)),
            access_token: Arc::new(RwLock::new(None)),
            user_id: Arc::new(RwLock::new(None)),
            room_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_homeserver(mut self, url: String) -> Self {
        self.homeserver_url = Arc::new(RwLock::new(Some(url)));
        self
    }

    pub fn with_access_token(mut self, token: String) -> Self {
        self.access_token = Arc::new(RwLock::new(Some(token)));
        self
    }

    pub fn with_user_id(mut self, user_id: String) -> Self {
        self.user_id = Arc::new(RwLock::new(Some(user_id)));
        self
    }

    pub fn with_room_id(mut self, room_id: String) -> Self {
        self.room_id = Arc::new(RwLock::new(Some(room_id)));
        self
    }

    /// 设置 Access Token
    pub async fn set_access_token(&self, token: String) {
        *self.access_token.write().await = Some(token);
    }

    /// 获取 Access Token
    pub async fn get_access_token(&self) -> Option<String> {
        self.access_token.read().await.clone()
    }
}

impl Default for MatrixAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Matrix Sync Response
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixSyncResponse {
    pub next_batch: String,
    pub rooms: Option<MatrixRooms>,
}

/// Matrix Rooms
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixRooms {
    pub join: Option<std::collections::HashMap<String, MatrixRoom>>,
}

/// Matrix Room
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixRoom {
    pub timeline: Option<MatrixTimeline>,
}

/// Matrix Timeline
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixTimeline {
    pub events: Option<Vec<MatrixEvent>>,
}

/// Matrix Event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub sender: Option<String>,
    pub content: MatrixContent,
    pub event_id: Option<String>,
    pub origin_server_ts: Option<u64>,
}

/// Matrix Event Content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixContent {
    pub body: Option<String>,
    pub msgtype: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

/// Matrix Room Message Send Request
#[derive(Debug, Serialize)]
pub struct MatrixRoomMessageRequest {
    pub msgtype: String,
    pub body: String,
}

/// Matrix Room Message Response
#[derive(Debug, Deserialize)]
pub struct MatrixRoomMessageResponse {
    pub event_id: String,
}

/// Matrix Sync Query Params
#[derive(Debug, Clone, Serialize)]
pub struct MatrixSyncQuery {
    pub access_token: String,
    #[serde(rename = "timeout")]
    pub timeout_ms: Option<u64>,
    pub since: Option<String>,
    pub filter: Option<String>,
}

/// Matrix Login Request
#[derive(Debug, Serialize)]
pub struct MatrixLoginRequest {
    pub identifier: MatrixUserIdentifier,
    pub initial_device_display_name: String,
    pub password: Option<String>,
    #[serde(rename = "type")]
    pub login_type: String,
}

#[derive(Debug, Serialize)]
pub struct MatrixUserIdentifier {
    #[serde(rename = "type")]
    pub user_id_type: String,
    pub user: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MatrixLoginResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
}

/// Incoming Matrix webhook payload (from outshot webhook integration)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixWebhookPayload {
    pub room_id: String,
    pub event_id: String,
    pub sender: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub content: MatrixWebhookContent,
    pub origin_server_ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixWebhookContent {
    pub body: String,
    pub msgtype: String,
}

#[async_trait]
impl PlatformAdapter for MatrixAdapter {
    fn platform_id(&self) -> &'static str {
        "matrix"
    }

    fn verify_webhook(&self, _request: &axum::extract::Request<axum::body::Body>) -> bool {
        // TODO: 实现 Matrix webhook 验证
        // 可以使用 HMAC 签名或简单 token 验证
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

        // 尝试解析为 webhook payload
        let webhook: Result<MatrixWebhookPayload, _> = serde_json::from_str(&body_str);

        if let Ok(payload) = webhook {
            let sender_id = payload.sender.clone();
            let session_id = format!("matrix:{}", payload.room_id);
            let timestamp = Utc.timestamp_millis_opt((payload.origin_server_ts as i64) / 1000)
                .single()
                .unwrap_or_else(|| Utc::now());
            let content = payload.content.body.clone();

            return Ok(InboundMessage {
                platform: "matrix".to_string(),
                sender_id,
                content,
                session_id,
                timestamp,
                raw: serde_json::to_value(&payload).unwrap_or_default(),
            });
        }

        // 尝试解析为标准 Matrix Event
        let event: Result<MatrixEvent, _> = serde_json::from_str(&body_str);

        if let Ok(event) = event {
            let sender_id = event.sender.clone().unwrap_or_default();
            let session_id = format!("matrix:{}", self.room_id.read().await.clone().unwrap_or_default());
            let timestamp = event
                .origin_server_ts
                .and_then(|ts| Utc.timestamp_millis_opt((ts as i64) / 1000).single())
                .unwrap_or_else(|| Utc::now());
            let content = event.content.body.clone().unwrap_or_default();

            return Ok(InboundMessage {
                platform: "matrix".to_string(),
                sender_id,
                content,
                session_id,
                timestamp,
                raw: serde_json::to_value(&event).unwrap_or_default(),
            });
        }

        Err(GatewayError::ParseError("Unknown Matrix payload format".to_string()))
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        // 提取 room_id 从 session_id
        let room_id = message.session_id.strip_prefix("matrix:").unwrap_or(&message.session_id);

        let msg_request = MatrixRoomMessageRequest {
            msgtype: "m.text".to_string(),
            body: response.content,
        };

        tracing::info!("Matrix outbound to room {}: {:?}", room_id, msg_request);
        // TODO: 实现实际发送逻辑 via Matrix Client-Server API
        let _ = room_id;
        Ok(())
    }
}

/// Matrix API 客户端 (用于主动发送消息)
pub struct MatrixClient {
    homeserver_url: String,
    access_token: Option<String>,
    http_client: reqwest::Client,
}

impl MatrixClient {
    pub fn new(homeserver_url: String) -> Self {
        Self {
            homeserver_url,
            access_token: None,
            http_client: Client::builder().build().unwrap_or_default(),
        }
    }

    pub fn with_access_token(mut self, token: String) -> Self {
        self.access_token = Some(token);
        self
    }

    /// 登录获取 Access Token
    pub async fn login(&mut self, user: &str, password: &str) -> Result<(), MatrixError> {
        let login_req = MatrixLoginRequest {
            identifier: MatrixUserIdentifier {
                user_id_type: "m.id.user".to_string(),
                user: Some(user.to_string()),
            },
            initial_device_display_name: "hermes-agent".to_string(),
            password: Some(password.to_string()),
            login_type: "m.login.password".to_string(),
        };

        let url = format!("{}/_matrix/client/r0/login", self.homeserver_url);
        let response = self
            .http_client
            .post(&url)
            .json(&login_req)
            .send()
            .await
            .map_err(|e| MatrixError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MatrixError::Authentication(
                response.status().to_string(),
            ));
        }

        let login_resp: MatrixLoginResponse =
            response.json().await.map_err(|e| MatrixError::Parse(e.to_string()))?;

        self.access_token = Some(login_resp.access_token);
        Ok(())
    }

    /// 发送房间消息
    pub async fn send_room_message(
        &self,
        room_id: &str,
        body: &str,
    ) -> Result<String, MatrixError> {
        let access_token = self.access_token.as_ref().ok_or(MatrixError::NotAuthenticated)?;

        let msg_req = MatrixRoomMessageRequest {
            msgtype: "m.text".to_string(),
            body: body.to_string(),
        };

        let txn_id = format!("hermes-{}", uuid::Uuid::new_v4());
        let url = format!(
            "{}/_matrix/client/r0/rooms/{}/send/m.room.message/{}?access_token={}",
            self.homeserver_url, room_id, txn_id, access_token
        );

        let response = self
            .http_client
            .put(&url)
            .json(&msg_req)
            .send()
            .await
            .map_err(|e| MatrixError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MatrixError::SendMessage(response.status().to_string()));
        }

        let msg_resp: MatrixRoomMessageResponse =
            response.json().await.map_err(|e| MatrixError::Parse(e.to_string()))?;

        Ok(msg_resp.event_id)
    }

    /// 同步最新事件
    pub async fn sync(&self, since: Option<&str>) -> Result<MatrixSyncResponse, MatrixError> {
        let access_token = self.access_token.as_ref().ok_or(MatrixError::NotAuthenticated)?;

        let mut url = format!(
            "{}/_matrix/client/r0/sync?access_token={}",
            self.homeserver_url, access_token
        );

        if let Some(since) = since {
            url.push_str(&format!("&since={}", since));
        }

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| MatrixError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(MatrixError::Sync(response.status().to_string()));
        }

        response
            .json()
            .await
            .map_err(|e| MatrixError::Parse(e.to_string()))
    }
}

/// Matrix 错误类型
#[derive(Debug, thiserror::Error)]
pub enum MatrixError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Authentication error: {0}")]
    Authentication(String),
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Send message error: {0}")]
    SendMessage(String),
    #[error("Sync error: {0}")]
    Sync(String),
}

type Client = reqwest::Client;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = MatrixAdapter::new();
        assert_eq!(adapter.platform_id(), "matrix");
    }

    #[test]
    fn test_matrix_client_new() {
        let client = MatrixClient::new("https://matrix.example.com".to_string());
        assert!(client.access_token.is_none());
    }

    #[test]
    fn test_matrix_client_with_token() {
        let client = MatrixClient::new("https://matrix.example.com".to_string())
            .with_access_token("test_token".to_string());
        assert!(client.access_token.is_some());
    }
}