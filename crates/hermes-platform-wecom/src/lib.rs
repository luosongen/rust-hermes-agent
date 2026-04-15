//! ## hermes-platform-wecom
//!
//! 企业微信（WeCom）平台适配器，将企业微信 Webhook 集成到 Hermes 网关。
//!
//! ### 功能概述
//! - **签名验证**：校验 URL 参数中的 `msg_signature`、`timestamp`、`nonce`
//! - **AES 解密**：使用 AES-256-CBC 解密消息体中的 `encrypt` 字段
//! - **入站解析**：解密后解析 XML 格式的 WeCom 消息
//! - **出站发送**：日志记录响应内容（当前为占位实现）
//!
//! ### 安全机制
//! 采用 AES-256-CBC 加密，需要以下配置：
//! - `token`：企业微信后台配置的 Token
//! - `aes_key`（Base64 编码）：43 位的 AES 密钥
//!
//! ### 解密流程
//! ```text
//! 密文（Base64）→ Base64解码 → 分离IV → CBC解密 → PKCS7去填充 → XML原文
//! ```
//!
//! ### 消息格式
//! - 会话 ID 格式：`wecom:{FromUserName}`

use aes::cipher::KeyInit;
use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use serde::{Deserialize, Serialize};
use sha1::Digest;

pub struct WeComAdapter {
    token: String,
    aes_key: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct WeComXMLRequest {
    #[serde(rename = "msg_signature")]
    msg_signature: String,
    timestamp: String,
    nonce: String,
    encrypt: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename = "xml")]
struct WeComDecrypted {
    #[serde(rename = "Content")]
    content: Option<String>,
    #[serde(rename = "FromUserName")]
    from_username: Option<String>,
}

impl WeComAdapter {
    pub fn new(_corp_id: String, _agent_id: String, token: String, aes_key_b64: String) -> Self {
        let aes_key = general_purpose::STANDARD
            .decode(aes_key_b64.as_bytes())
            .expect("invalid base64 aes_key");
        Self {
            token,
            aes_key,
        }
    }

    fn verify_signature(&self, sig: &str, timestamp: &str, nonce: &str, encrypt: &str) -> bool {
        let mut parts = vec![&self.token, timestamp, nonce, encrypt];
        parts.sort();
        let joined = parts.join("");
        hex_sha1(&joined) == sig
    }

    fn decrypt(&self, encrypt_b64: &str) -> Result<String, GatewayError> {
        use aes::Aes256;
        use aes::cipher::BlockDecrypt;

        let ciphertext = general_purpose::STANDARD
            .decode(encrypt_b64.as_bytes())
            .map_err(|e| GatewayError::ParseError(format!("base64 decode: {}", e)))?;

        if ciphertext.len() < 16 {
            return Err(GatewayError::ParseError("ciphertext too short".into()));
        }

        let (ciphertext_body, iv) = ciphertext.split_at(ciphertext.len() - 16);

        let key = aes::cipher::Key::<Aes256>::from_slice(&self.aes_key);
        let cipher = Aes256::new(key);

        let mut ct = ciphertext_body.to_vec();
        let block_size = 16;

        // CBC decrypt: decrypt block, then XOR with previous ciphertext (IV for first)
        for i in (0..ct.len() / block_size).rev() {
            let offset = i * block_size;
            let mut block: aes::cipher::Block<Aes256> =
                aes::cipher::Block::<Aes256>::from_exact_iter(ct[offset..offset + block_size].iter().copied())
                    .unwrap();
            cipher.decrypt_block(&mut block);
            // XOR with previous ciphertext block (IV for first block)
            let prev: [u8; 16] = if i == 0 {
                iv.try_into().unwrap()
            } else {
                ct[offset - block_size..offset].try_into().unwrap()
            };
            for j in 0..block_size {
                ct[offset + j] = block[j] ^ prev[j];
            }
        }

        // Remove PKCS7 padding (last byte = pad count)
        let pad = ct[ct.len() - 1] as usize;
        if pad == 0 || pad > 16 || ct.len() < pad {
            return Err(GatewayError::ParseError("invalid padding".into()));
        }
        ct.truncate(ct.len() - pad);

        let xml_str = String::from_utf8(ct)
            .map_err(|e| GatewayError::ParseError(format!("utf8: {}", e)))?;
        Ok(xml_str)
    }
}

fn hex_sha1(s: &str) -> String {
    let mut hasher = sha1::Sha1::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

#[async_trait]
impl PlatformAdapter for WeComAdapter {
    fn platform_id(&self) -> &str {
        "wecom"
    }

    fn verify_webhook(&self, request: &Request<Body>) -> bool {
        let query = request.uri().query().unwrap_or("");
        query.contains("msg_signature")
            && query.contains("timestamp")
            && query.contains("nonce")
    }

    async fn parse_inbound(
        &self,
        request: Request<Body>,
    ) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;
        let body_str = String::from_utf8_lossy(&body).to_string();

        let params: WeComXMLRequest =
            serde_xml_rs::from_str(&body_str)
                .map_err(|e| GatewayError::ParseError(format!("wecom xml parse: {}", e)))?;

        if !self.verify_signature(
            &params.msg_signature,
            &params.timestamp,
            &params.nonce,
            &params.encrypt,
        ) {
            return Err(GatewayError::VerificationFailed(
                "WeCom signature mismatch".into(),
            ));
        }

        let xml_str = self.decrypt(&params.encrypt)?;

        let decrypted: WeComDecrypted =
            serde_xml_rs::from_str(&xml_str)
                .map_err(|e| GatewayError::ParseError(format!("wecom decrypted xml parse: {}", e)))?;

        let content = decrypted.content.clone().unwrap_or_default();
        let sender_id = decrypted.from_username.clone().unwrap_or_default();
        let session_id = format!("wecom:{}", sender_id);

        Ok(InboundMessage {
            platform: "wecom".into(),
            sender_id,
            content,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::to_value(&decrypted).unwrap_or_default(),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        tracing::info!(
            "WeCom response to {}: {}",
            message.sender_id,
            response.content
        );
        Ok(())
    }
}
