//! 飞书 API 客户端模块
//!
//! 提供与飞书开放平台 API 交互的功能，包括：
//! - 获取 Tenant Access Token
//! - 发送消息

use crate::error::FeishuError;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 飞书 API 客户端
pub struct FeishuClient {
    app_id: String,
    app_secret: String,
    access_token: Arc<RwLock<Option<String>>>,
    http_client: reqwest::Client,
}

impl FeishuClient {
    /// 创建新的飞书客户端
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            access_token: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::new(),
        }
    }

    /// 获取 Access Token
    /// 优先使用缓存的 token，若无则重新获取
    pub async fn get_access_token(&self) -> Result<String, FeishuError> {
        // 尝试从缓存获取
        if let Some(token) = self.access_token.read().await.clone() {
            return Ok(token);
        }

        let url = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";
        let body = serde_json::json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret
        });

        let response = self
            .http_client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| FeishuError::Api(e.to_string()))?;

        #[derive(Deserialize)]
        struct TokenResponse {
            code: i32,
            msg: String,
            tenant_access_token: String,
        }

        let token_resp: TokenResponse = response
            .json()
            .await
            .map_err(|e| FeishuError::Parse(e.to_string()))?;

        if token_resp.code != 0 {
            return Err(FeishuError::Auth(token_resp.msg));
        }

        // 缓存 token
        *self.access_token.write().await = Some(token_resp.tenant_access_token.clone());
        Ok(token_resp.tenant_access_token)
    }

    /// 发送消息
    ///
    /// - `receive_id`: 接收者 ID（通常为 chat_id）
    /// - `msg_type`: 消息类型（text、post、image 等）
    /// - `content`: 消息内容（JSON 字符串格式）
    pub async fn send_message(
        &self,
        receive_id: &str,
        msg_type: &str,
        content: &str,
    ) -> Result<String, FeishuError> {
        let token = self.get_access_token().await?;
        let url = "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id";

        let body = serde_json::json!({
            "receive_id": receive_id,
            "msg_type": msg_type,
            "content": serde_json::json!(content)
        });

        let response = self
            .http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await
            .map_err(|e| FeishuError::Api(e.to_string()))?;

        #[derive(Deserialize)]
        struct SendResponse {
            code: i32,
            msg: String,
            data: Option<SendData>,
        }

        #[derive(Deserialize)]
        struct SendData {
            message_id: String,
        }

        let resp: SendResponse = response
            .json()
            .await
            .map_err(|e| FeishuError::Parse(e.to_string()))?;

        if resp.code != 0 {
            return Err(FeishuError::Api(resp.msg));
        }

        resp.data
            .map(|d| d.message_id)
            .ok_or_else(|| FeishuError::Api("No message_id returned".to_string()))
    }
}
