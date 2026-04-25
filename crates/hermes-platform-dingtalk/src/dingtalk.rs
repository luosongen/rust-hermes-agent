//! 钉钉平台适配器模块

use crate::client::DingTalkStreamClient;
use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 钉钉适配器
pub struct DingTalkAdapter {
    client_id: Arc<RwLock<Option<String>>>,
    client_secret: Arc<RwLock<Option<String>>>,
    stream_client: Arc<RwLock<Option<DingTalkStreamClient>>>,
    session_webhooks: Arc<RwLock<std::collections::HashMap<String, (String, i64)>>>,
}

impl DingTalkAdapter {
    /// 创建新的钉钉适配器
    pub fn new() -> Self {
        Self {
            client_id: Arc::new(RwLock::new(None)),
            client_secret: Arc::new(RwLock::new(None)),
            stream_client: Arc::new(RwLock::new(None)),
            session_webhooks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// 使用凭证创建适配器
    pub fn with_credentials(mut self, client_id: String, client_secret: String) -> Self {
        self.client_id = Arc::new(RwLock::new(Some(client_id)));
        self.client_secret = Arc::new(RwLock::new(Some(client_secret)));
        self
    }

    /// 设置凭证
    pub async fn set_credentials(&self, client_id: String, client_secret: String) {
        *self.client_id.write().await = Some(client_id);
        *self.client_secret.write().await = Some(client_secret);
    }
}

impl Default for DingTalkAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// 钉钉文本消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkText {
    pub content: String,
}

/// 钉钉回调消息
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DingTalkCallback {
    pub msg_id: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_type: Option<String>,
    pub sender_id: Option<String>,
    pub sender_nick: Option<String>,
    pub session_webhook: Option<String>,
    pub text: Option<DingTalkText>,
    pub robot_code: Option<String>,
    pub create_at: Option<i64>,
    #[serde(rename = "isInAtList")]
    pub is_in_at_list: Option<bool>,
    /// 图片消息的图片URL
    pub picture_url: Option<String>,
    /// 音频消息的语音URL
    pub voice_url: Option<String>,
    /// 视频消息的视频URL
    pub video_url: Option<String>,
}

#[async_trait]
impl PlatformAdapter for DingTalkAdapter {
    fn platform_id(&self) -> &'static str {
        "dingtalk"
    }

    fn verify_webhook(&self, _request: &axum::extract::Request<axum::body::Body>) -> bool {
        // 钉钉 Stream Mode 不使用 webhook 验证
        true
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let callback: DingTalkCallback = serde_json::from_slice(&body)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        let sender_id = callback
            .sender_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        let chat_id = callback
            .conversation_id
            .clone()
            .unwrap_or_else(|| sender_id.clone());

        let session_id = format!("dingtalk:{}", chat_id);

        // 存储 session webhook
        if let Some(webhook) = &callback.session_webhook {
            let expired_time = callback.create_at.unwrap_or(0) + 7200 * 1000;
            self.session_webhooks
                .write()
                .await
                .insert(chat_id.clone(), (webhook.clone(), expired_time));
        }

        // 构建消息内容，支持文本、图片、音频、视频多种消息类型
        let content = if let Some(text) = &callback.text {
            text.content.clone()
        } else if let Some(picture_url) = &callback.picture_url {
            format!("[图片] {}", picture_url)
        } else if let Some(voice_url) = &callback.voice_url {
            format!("[音频] {}", voice_url)
        } else if let Some(video_url) = &callback.video_url {
            format!("[视频] {}", video_url)
        } else {
            String::new()
        };

        Ok(InboundMessage {
            platform: "dingtalk".to_string(),
            sender_id,
            content,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::to_value(&callback).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let chat_id = message
            .session_id
            .strip_prefix("dingtalk:")
            .unwrap_or(&message.session_id);

        let webhook_info = self.session_webhooks.read().await.get(chat_id).cloned();

        let (session_webhook, _) = webhook_info
            .ok_or_else(|| GatewayError::OutboundError("No session webhook available".to_string()))?;

        let normalized = normalize_markdown(&response.content);
        let payload = serde_json::json!({
            "msgtype": "markdown",
            "markdown": {
                "title": "Hermes",
                "text": normalized
            }
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(&session_webhook)
            .json(&payload)
            .send()
            .await
            .map_err(|e| GatewayError::OutboundError(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(GatewayError::OutboundError(format!(
                "Send failed: {}",
                resp.status()
            )));
        }

        Ok(())
    }
}

/// 标准化 Markdown (适配钉钉渲染)
fn normalize_markdown(text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut prev_was_blank = true;

    for line in &lines {
        let trimmed = line.trim();
        // 编号列表前需要空行
        if trimmed.starts_with(|c: char| c.is_ascii_digit()) && trimmed.contains('.')
            && !prev_was_blank && !result.is_empty()
        {
            result.push("");
        }
        result.push(line);
        prev_was_blank = trimmed.is_empty();
    }

    result.join("\n")
}
