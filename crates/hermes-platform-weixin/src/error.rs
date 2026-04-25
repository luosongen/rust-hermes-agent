use thiserror::Error;

/// 微信平台错误类型
#[derive(Error, Debug)]
pub enum WeixinError {
    #[error("认证失败: {0}")]
    Auth(String),

    #[error("API 错误: {0}")]
    Api(String),

    #[error("未认证")]
    NotAuthenticated,

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("加密错误: {0}")]
    Encrypt(String),

    #[error("网络错误: {0}")]
    Network(String),
}
