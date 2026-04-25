//! ## hermes-platform-feishu
//!
//! 飞书（Feishu/Lark）平台适配器，将飞书 Webhook 集成到 Hermes 网关。
//!
//! ### 功能概述
//! - **Webhook 验证**：校验 URL 查询参数中的签名
//! - **入站解析**：将飞书事件 JSON 解析为 `InboundMessage`
//! - **出站发送**：通过飞书 IM API 发送消息
//!
//! ### 配置要求
//! 创建适配器时需提供：
//! - `app_id`：飞书应用的 App ID
//! - `app_secret`：飞书应用的 App Secret
//! - `encrypt_key`（可选）：用于解密消息体的加密密钥
//!
//! ### 消息格式
//! - 会话 ID 格式：`feishu:{chat_id}`

use thiserror::Error;

#[derive(Error, Debug)]
pub enum FeishuError {
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Missing credential: {0}")]
    MissingCredential(String),
    #[error("Encrypt error: {0}")]
    Encrypt(String),
    #[error("Signature verify failed")]
    SignatureVerifyFailed,
}
