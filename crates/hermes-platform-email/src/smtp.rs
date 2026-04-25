//! SMTP client for sending emails

/// SMTP configuration
#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub use_tls: bool,
}

/// SMTP client
pub struct SmtpClient {
    config: SmtpConfig,
}

impl SmtpClient {
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }
}
