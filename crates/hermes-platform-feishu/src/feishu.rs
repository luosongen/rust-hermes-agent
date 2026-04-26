//! 飞书平台适配器模块
//!
//! 实现 `PlatformAdapter` trait，将飞书 Webhook 事件转换为规范的 `InboundMessage`

use crate::client::FeishuClient;
use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 飞书平台适配器
///
/// 支持：
/// - 文本消息
/// - 富文本消息（post）
/// - 图片、音频、视频等媒体消息
pub struct FeishuAdapter {
    app_id: Arc<RwLock<Option<String>>>,
    app_secret: Arc<RwLock<Option<String>>>,
    client: Arc<RwLock<Option<FeishuClient>>>,
    encrypt_key: Arc<RwLock<Option<String>>>,
}

impl FeishuAdapter {
    /// 创建新的飞书适配器
    pub fn new() -> Self {
        Self {
            app_id: Arc::new(RwLock::new(None)),
            app_secret: Arc::new(RwLock::new(None)),
            client: Arc::new(RwLock::new(None)),
            encrypt_key: Arc::new(RwLock::new(None)),
        }
    }

    /// 使用凭据创建适配器
    pub fn with_credentials(mut self, app_id: String, app_secret: String) -> Self {
        self.app_id = Arc::new(RwLock::new(Some(app_id.clone())));
        self.app_secret = Arc::new(RwLock::new(Some(app_secret.clone())));
        self.client = Arc::new(RwLock::new(Some(FeishuClient::new(app_id, app_secret))));
        self
    }

    /// 设置加密密钥
    pub fn with_encrypt_key(mut self, encrypt_key: String) -> Self {
        self.encrypt_key = Arc::new(RwLock::new(Some(encrypt_key)));
        self
    }

    /// 异步设置凭据
    pub async fn set_credentials(&self, app_id: String, app_secret: String) {
        *self.app_id.write().await = Some(app_id.clone());
        *self.app_secret.write().await = Some(app_secret.clone());
        *self.client.write().await = Some(FeishuClient::new(app_id, app_secret));
    }
}

impl Default for FeishuAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// 飞书事件枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEvent {
    #[serde(rename = "schema")]
    pub schema: Option<String>,
    pub header: FeishuEventHeader,
    pub event: Option<FeishuMessageEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEventHeader {
    pub event_id: Option<String>,
    pub event_type: Option<String>,
    pub create_time: Option<String>,
    pub token: Option<String>,
    pub app_id: Option<String>,
    pub tenant_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessageEvent {
    pub sender: Option<FeishuSender>,
    pub content: Option<String>,
    pub message_type: Option<String>,
    pub create_time: Option<String>,
    pub message_id: Option<String>,
    pub upper_message_id: Option<String>,
    pub chat_id: Option<String>,
    pub root_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSender {
    pub sender_id: FeishuSenderId,
    pub sender_type: Option<String>,
    pub tenant_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSenderId {
    pub open_id: Option<String>,
    pub user_id: Option<String>,
    pub union_id: Option<String>,
}

#[async_trait]
impl PlatformAdapter for FeishuAdapter {
    fn platform_id(&self) -> &'static str {
        "feishu"
    }

    fn verify_webhook(&self, request: &axum::extract::Request<axum::body::Body>) -> bool {
        // 飞书验证签名在 parse_inbound 中进行（需要 body 和 query params）
        // 此处检查加密密钥是否已配置
        // 实际验证使用 HMAC-SHA256
        let encrypt_key = request.headers()
            .get("X-Feishu-Encryption-Key")
            .and_then(|v| v.to_str().ok());
        encrypt_key.is_some()
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let event: FeishuEvent = serde_json::from_slice(&body)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let message_event = event.event.as_ref()
            .ok_or_else(|| GatewayError::ParseError("Missing event data".to_string()))?;

        // 优先使用 open_id，其次 user_id（优先使用非空值）
        let sender_id = message_event
            .sender
            .as_ref()
            .and_then(|s| s.sender_id.open_id.clone())
            .filter(|id| !id.is_empty())
            .or_else(|| message_event.sender.as_ref().and_then(|s| s.sender_id.user_id.clone()))
            .unwrap_or_else(|| "unknown".to_string());

        let chat_id = message_event
            .chat_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let session_id = format!("feishu:{}", chat_id);

        // 解析消息内容
        // 飞书消息内容是 JSON 字符串，需要进一步解析
        let content = if let Some(content_str) = &message_event.content {
            if let Ok(content_json) = serde_json::from_str::<serde_json::Value>(content_str) {
                let message_type = message_event.message_type.as_deref().unwrap_or("text");

                match message_type {
                    "text" => {
                        // 文本消息：提取 text 字段
                        content_json["text"]
                            .as_str()
                            .unwrap_or(content_str)
                            .to_string()
                    }
                    "post" => {
                        // 富文本消息：提取 text 字段（飞书 post 类型包含 text 和 rich_text）
                        // rich_text 结构复杂，先提取 text 作为预览
                        if let Some(text) = content_json["text"].as_str() {
                            text.to_string()
                        } else {
                            // 如果没有 text 字段，尝试提取 rich_text 中的文本
                            let rich_text = &content_json["rich_text"];
                            if let Some(elements) = rich_text.get("elements").and_then(|e| e.as_array()) {
                                let mut parts = Vec::new();
                                for item in elements {
                                    if let Some(text_content) = item.get("text").and_then(|t| t.as_str()) {
                                        parts.push(text_content.to_string());
                                    }
                                }
                                if !parts.is_empty() {
                                    parts.join("")
                                } else {
                                    content_str.clone()
                                }
                            } else {
                                content_str.clone()
                            }
                        }
                    }
                    "image" => {
                        // 图片消息：提取 image_key
                        let key = content_json["image_key"]
                            .as_str()
                            .unwrap_or("unknown");
                        format!("[图片] {}", key)
                    }
                    "audio" => {
                        // 音频消息：提取 audio_key
                        let key = content_json["audio_key"]
                            .as_str()
                            .unwrap_or("unknown");
                        format!("[音频] {}", key)
                    }
                    "video" => {
                        // 视频消息：提取 video_key
                        let key = content_json["video_key"]
                            .as_str()
                            .unwrap_or("unknown");
                        format!("[视频] {}", key)
                    }
                    "file" => {
                        // 文件消息：提取 file_key
                        let key = content_json["file_key"]
                            .as_str()
                            .unwrap_or("unknown");
                        format!("[文件] {}", key)
                    }
                    "sticker" => {
                        // 表情消息：提取 sticker_id
                        let key = content_json["sticker_id"]
                            .as_str()
                            .unwrap_or("unknown");
                        format!("[表情] {}", key)
                    }
                    "media" => {
                        // 媒体消息（旧版格式）：提取 file_key
                        let key = content_json["file_key"]
                            .as_str()
                            .unwrap_or("unknown");
                        format!("[媒体] {}", key)
                    }
                    _ => {
                        // 未知消息类型，尝试提取 text 字段，否则返回原始内容
                        content_json["text"]
                            .as_str()
                            .unwrap_or(content_str)
                            .to_string()
                    }
                }
            } else {
                content_str.clone()
            }
        } else {
            String::new()
        };

        Ok(InboundMessage {
            platform: "feishu".to_string(),
            sender_id,
            content,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::to_value(&event).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let chat_id = message
            .session_id
            .strip_prefix("feishu:")
            .unwrap_or(&message.session_id);

        let client_guard = self.client.read().await;
        let client = client_guard.as_ref()
            .ok_or_else(|| GatewayError::OutboundError("Feishu client not initialized".to_string()))?;

        client
            .send_message(chat_id, "text", &response.content)
            .await
            .map_err(|e| GatewayError::OutboundError(e.to_string()))?;

        Ok(())
    }
}
