//! 钉钉 API 客户端模块

use crate::error::DingTalkError;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 钉钉 Stream Mode 客户端
pub struct DingTalkStreamClient {
    client_id: String,
    client_secret: String,
    access_token: Arc<RwLock<Option<String>>>,
    http_client: reqwest::Client,
}

impl DingTalkStreamClient {
    /// 创建新的 Stream 客户端
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            access_token: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::new(),
        }
    }

    /// 获取 Access Token
    pub async fn get_access_token(&self) -> Result<String, DingTalkError> {
        // 先尝试从缓存读取
        if let Some(token) = self.access_token.read().await.clone() {
            return Ok(token);
        }

        let url = "https://api.dingtalk.com/v1.0/oauth2/accessToken";
        let body = serde_json::json!({
            "appKey": self.client_id,
            "appSecret": self.client_secret
        });

        let response = self
            .http_client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| DingTalkError::Api(e.to_string()))?;

        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct TokenResponse {
            access_token: String,
            expire_in: u64,
        }

        let token_resp: TokenResponse = response
            .json()
            .await
            .map_err(|e| DingTalkError::Parse(e.to_string()))?;

        // 缓存 token
        *self.access_token.write().await = Some(token_resp.access_token.clone());
        Ok(token_resp.access_token)
    }
}
