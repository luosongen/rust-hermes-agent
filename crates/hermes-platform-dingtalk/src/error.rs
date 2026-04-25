//! 钉钉平台错误类型

use thiserror::Error;

/// 钉钉平台适配器错误类型
#[derive(Error, Debug)]
pub enum DingTalkError {
    #[error("认证失败: {0}")]
    Auth(String),

    #[error("API 错误: {0}")]
    Api(String),

    #[error("未认证")]
    NotAuthenticated,

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("流式错误: {0}")]
    Stream(String),

    #[error("WebSocket 错误: {0}")]
    WebSocket(String),

    #[error("缺少凭证: {0}")]
    MissingCredential(String),
}
