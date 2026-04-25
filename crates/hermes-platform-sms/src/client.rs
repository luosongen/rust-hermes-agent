//! Twilio SMS API Client
//!
//! 使用 Twilio REST API 发送短信

use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::SmsError;

/// Twilio API 客户端
#[derive(Clone)]
pub struct TwilioClient {
    account_sid: String,
    auth_token: String,
    from_number: Option<String>,
    http_client: Client,
}

impl TwilioClient {
    /// 创建新的 Twilio 客户端
    pub fn new(account_sid: String, auth_token: String) -> Self {
        Self {
            account_sid,
            auth_token,
            from_number: None,
            http_client: Client::new(),
        }
    }

    /// 设置发件人号码
    pub fn with_from(mut self, from_number: String) -> Self {
        self.from_number = Some(from_number);
        self
    }

    /// 获取 Basic Auth 头
    fn auth_header(&self) -> String {
        let credentials = format!("{}:{}", self.account_sid, self.auth_token);
        format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(credentials))
    }

    /// Twilio API 基础 URL
    fn api_base_url(&self) -> String {
        format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}",
            self.account_sid
        )
    }

    /// 发送短信
    pub async fn send_message(
        &self,
        to: &str,
        body: &str,
    ) -> Result<TwilioMessageResponse, SmsError> {
        let from = self
            .from_number
            .as_ref()
            .ok_or_else(|| SmsError::NotAuthenticated)?;

        let url = format!("{}/Messages.json", self.api_base_url());

        let params = [
            ("To", to),
            ("From", from),
            ("Body", body),
        ];

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .map_err(|e| SmsError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(SmsError::SendMessage(format!(
                "status: {}, body: {}",
                status, body
            )));
        }

        response
            .json::<TwilioMessageResponse>()
            .await
            .map_err(|e| SmsError::Parse(e.to_string()))
    }

    /// 验证 Twilio Webhook 签名
    pub fn verify_signature(
        &self,
        url: &str,
        params: &[(String, String)],
        signature: &str,
    ) -> bool {
        // 构建签名验证字符串：URL + 按键名排序的参数
        let mut sign_data = url.to_string();
        let mut sorted_params: Vec<_> = params.iter().collect();
        sorted_params.sort_by(|a, b| a.0.cmp(&b.0));

        for (key, value) in sorted_params {
            sign_data.push_str(key);
            sign_data.push_str(value);
        }

        // 使用 HMAC-SHA1 计算签名
        use hmac::{Hmac, Mac};
        type HmacSha1 = Hmac<sha1::Sha1>;

        let mut mac = HmacSha1::new_from_slice(self.auth_token.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(sign_data.as_bytes());

        let expected = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

        // 常数时间比较
        constant_time_compare(&expected, signature)
    }
}

/// Twilio 消息响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwilioMessageResponse {
    pub sid: String,
    pub to: String,
    pub from: String,
    pub body: String,
    pub status: String,
    pub date_created: Option<String>,
    pub date_sent: Option<String>,
    pub num_segments: Option<String>,
    #[serde(rename = "uri")]
    pub uri: Option<String>,
}

/// Twilio Webhook Payload (Form-encoded)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwilioWebhookPayload {
    #[serde(rename = "ToCountry")]
    pub to_country: Option<String>,
    #[serde(rename = "ToState")]
    pub to_state: Option<String>,
    #[serde(rename = "SmsMessageSid")]
    pub sms_message_sid: Option<String>,
    #[serde(rename = "NumMedia")]
    pub num_media: Option<String>,
    #[serde(rename = "ToCity")]
    pub to_city: Option<String>,
    #[serde(rename = "FromZip")]
    pub from_zip: Option<String>,
    #[serde(rename = "SmsSid")]
    pub sms_sid: Option<String>,
    #[serde(rename = "FromCity")]
    pub from_city: Option<String>,
    #[serde(rename = "FromCountry")]
    pub from_country: Option<String>,
    #[serde(rename = "To")]
    pub to: Option<String>,
    #[serde(rename = "ToZip")]
    pub to_zip: Option<String>,
    #[serde(rename = "FromState")]
    pub from_state: Option<String>,
    #[serde(rename = "Body")]
    pub body: Option<String>,
    #[serde(rename = "From")]
    pub from: Option<String>,
    #[serde(rename = "ApiVersion")]
    pub api_version: Option<String>,
    #[serde(rename = "MessageSid")]
    pub message_sid: Option<String>,
    #[serde(rename = "AccountSid")]
    pub account_sid: Option<String>,
}

/// 常数时间字符串比较，防止时序攻击
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("hello", "hello"));
        assert!(!constant_time_compare("hello", "world"));
        assert!(!constant_time_compare("hello", "hell"));
        assert!(!constant_time_compare("hello", "helloo"));
    }

    #[test]
    fn test_twilio_client_new() {
        let client = TwilioClient::new(
            "AC123".to_string(),
            "auth_token".to_string(),
        );
        assert!(client.from_number.is_none());
    }

    #[test]
    fn test_twilio_client_with_from() {
        let client = TwilioClient::new(
            "AC123".to_string(),
            "auth_token".to_string(),
        )
        .with_from("+1234567890".to_string());
        assert!(client.from_number.is_some());
    }
}
