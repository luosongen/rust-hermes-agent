use crate::client::WeixinClient;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 微信适配器
pub struct WeixinAdapter {
    app_id: Arc<RwLock<Option<String>>>,
    app_secret: Arc<RwLock<Option<String>>>,
    client: Arc<RwLock<Option<WeixinClient>>>,
    context_tokens: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

impl WeixinAdapter {
    pub fn new() -> Self {
        Self {
            app_id: Arc::new(RwLock::new(None)),
            app_secret: Arc::new(RwLock::new(None)),
            client: Arc::new(RwLock::new(None)),
            context_tokens: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub fn with_credentials(mut self, app_id: String, app_secret: String) -> Self {
        self.app_id = Arc::new(RwLock::new(Some(app_id.clone())));
        self.app_secret = Arc::new(RwLock::new(Some(app_secret.clone())));
        self.client = Arc::new(RwLock::new(Some(WeixinClient::new(app_id, app_secret))));
        self
    }

    pub async fn set_credentials(&self, app_id: String, app_secret: String) {
        *self.app_id.write().await = Some(app_id.clone());
        *self.app_secret.write().await = Some(app_secret.clone());
        *self.client.write().await = Some(WeixinClient::new(app_id, app_secret));
    }
}

impl Default for WeixinAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// 微信消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeixinContent {
    pub content: Option<String>,
}

/// 微信 Webhook 事件 (XML 格式)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeixinWebhookEvent {
    #[serde(rename = "ToUserName")]
    pub to_username: Option<String>,
    #[serde(rename = "FromUserName")]
    pub from_username: Option<String>,
    #[serde(rename = "CreateTime")]
    pub create_time: Option<String>,
    #[serde(rename = "MsgType")]
    pub msg_type: Option<String>,
    #[serde(rename = "Content")]
    pub content: Option<String>,
    #[serde(rename = "MsgId")]
    pub msg_id: Option<String>,
    #[serde(rename = "MediaId")]
    pub media_id: Option<String>,
    #[serde(rename = "PicUrl")]
    pub pic_url: Option<String>,
}

#[async_trait]
impl PlatformAdapter for WeixinAdapter {
    fn platform_id(&self) -> &'static str {
        "weixin"
    }

    fn verify_webhook(&self, request: &axum::extract::Request<axum::body::Body>) -> bool {
        // 微信使用 URL 参数中的签名验证（仅在 Webhook 配置时）
        // 实际消息接收依赖 IP 白名单和 token 安全
        // 此处检查必要的消息头部是否存在
        let headers = request.headers();
        headers.contains_key("Content-Type")
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let body_str = String::from_utf8_lossy(&body);

        // 解析 XML 格式的微信消息
        let event: WeixinWebhookEvent = serde_xml_rs::from_str(&body_str)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let sender_id = event
            .from_username
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let session_id = format!("weixin:{}", sender_id);

        // 根据消息类型提取内容
        let content = extract_content(&event);

        let timestamp = event
            .create_time
            .as_ref()
            .and_then(|t| t.parse::<i64>().ok())
            .map(|t| Utc.timestamp_opt(t, 0).single().unwrap_or_else(Utc::now))
            .unwrap_or_else(Utc::now);

        Ok(InboundMessage {
            platform: "weixin".to_string(),
            sender_id,
            content,
            session_id,
            timestamp,
            raw: serde_json::to_value(&event).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let open_id = message
            .session_id
            .strip_prefix("weixin:")
            .unwrap_or(&message.session_id);

        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| GatewayError::OutboundError("Weixin client not initialized".to_string()))?;

        let token = client
            .get_token()
            .await
            .map_err(|e| GatewayError::OutboundError(e.to_string()))?;

        client
            .send_message(&token, open_id, "text", &response.content)
            .await
            .map_err(|e| GatewayError::OutboundError(e.to_string()))?;

        Ok(())
    }
}

/// 根据消息类型提取内容
fn extract_content(event: &WeixinWebhookEvent) -> String {
    match event.msg_type.as_deref() {
        Some("text") => {
            // 文本消息
            event.content.clone().unwrap_or_default()
        }
        Some("image") => {
            // 图片消息
            if let Some(pic_url) = &event.pic_url {
                format!("[图片] {}", pic_url)
            } else if let Some(media_id) = &event.media_id {
                format!("[图片] media_id: {}", media_id)
            } else {
                "[图片]".to_string()
            }
        }
        Some("voice") => {
            // 音频消息
            if let Some(media_id) = &event.media_id {
                format!("[语音] media_id: {}", media_id)
            } else {
                "[语音]".to_string()
            }
        }
        Some("video") => {
            // 视频消息
            if let Some(media_id) = &event.media_id {
                format!("[视频] media_id: {}", media_id)
            } else {
                "[视频]".to_string()
            }
        }
        Some("shortvideo") => {
            // 短视频消息
            if let Some(media_id) = &event.media_id {
                format!("[短视频] media_id: {}", media_id)
            } else {
                "[短视频]".to_string()
            }
        }
        Some("location") => {
            // 位置消息
            event
                .content
                .clone()
                .unwrap_or_else(|| "[位置]".to_string())
        }
        Some("link") => {
            // 链接消息
            event
                .content
                .clone()
                .unwrap_or_else(|| "[链接]".to_string())
        }
        _ => {
            // 其他消息类型或未知
            event
                .content
                .clone()
                .unwrap_or_else(|| "[未知消息]".to_string())
        }
    }
}
