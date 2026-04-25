use crate::error::WeixinError;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 微信 API 客户端 (iLink Bot API)
pub struct WeixinClient {
    app_id: String,
    app_secret: String,
    token: Arc<RwLock<Option<String>>>,
    http_client: reqwest::Client,
}

impl WeixinClient {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            token: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::new(),
        }
    }

    /// 获取 Access Token
    pub async fn get_token(&self) -> Result<String, WeixinError> {
        if let Some(token) = self.token.read().await.clone() {
            return Ok(token);
        }

        let url = format!(
            "https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid={}&secret={}",
            self.app_id, self.app_secret
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| WeixinError::Network(e.to_string()))?;

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: Option<String>,
            expires_in: Option<u64>,
            errcode: Option<i32>,
            errmsg: Option<String>,
        }

        let token_resp: TokenResponse = response
            .json()
            .await
            .map_err(|e| WeixinError::Parse(e.to_string()))?;

        if let Some(errcode) = token_resp.errcode {
            if errcode != 0 {
                return Err(WeixinError::Api(
                    token_resp.errmsg.unwrap_or_else(|| errcode.to_string()),
                ));
            }
        }

        let token = token_resp
            .access_token
            .ok_or_else(|| WeixinError::Auth("未返回 access token".to_string()))?;

        *self.token.write().await = Some(token.clone());
        Ok(token)
    }

    /// 发送消息
    pub async fn send_message(
        &self,
        token: &str,
        to_user: &str,
        msg_type: &str,
        content: &str,
    ) -> Result<(), WeixinError> {
        let url = format!(
            "https://api.weixin.qq.com/cgi-bin/message/custom/send?access_token={}",
            token
        );

        let body = serde_json::json!({
            "touser": to_user,
            "msgtype": msg_type,
            msg_type: {
                "content": content
            }
        });

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| WeixinError::Network(e.to_string()))?;

        #[derive(Deserialize)]
        struct SendResponse {
            errcode: Option<i32>,
            errmsg: Option<String>,
        }

        let resp: SendResponse = response
            .json()
            .await
            .map_err(|e| WeixinError::Parse(e.to_string()))?;

        if let Some(errcode) = resp.errcode {
            if errcode != 0 {
                return Err(WeixinError::Api(
                    resp.errmsg.unwrap_or_else(|| errcode.to_string()),
                ));
            }
        }

        Ok(())
    }
}

/// 微信消息
#[derive(Debug, Clone)]
pub struct WeixinMessage {
    pub msg_id: Option<String>,
    pub msg_type: Option<String>,
    pub from_username: Option<String>,
    pub create_time: Option<u64>,
    pub content: Option<String>,
}
