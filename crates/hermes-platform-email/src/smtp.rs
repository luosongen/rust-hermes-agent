//! SMTP client for sending emails

use crate::error::EmailError;

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

/// SMTP client for sending emails
pub struct SmtpClient {
    config: SmtpConfig,
}

impl SmtpClient {
    /// Create a new SMTP client with the given configuration
    pub fn new(config: SmtpConfig) -> Self {
        Self { config }
    }

    /// Send an email via SMTP
    #[allow(dead_code)]
    pub async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError> {
        use lettre::message::Message;
        use lettre::transport::smtp::AsyncSmtpTransport;
        use lettre::transport::smtp::authentication::Credentials;
        use lettre::Tokio1Executor;
        use lettre::AsyncTransport;

        // Build the email message using Mailbox
        let from_addr = self.config.from_address.parse()
            .map_err(|e| EmailError::ParseError(format!("Invalid from address: {}", e)))?;
        let to_addr = to.parse()
            .map_err(|e| EmailError::ParseError(format!("Invalid to address: {}", e)))?;

        let email = Message::builder()
            .from(from_addr)
            .to(to_addr)
            .subject(subject)
            .body(String::from(body))
            .map_err(|e| EmailError::ParseError(format!("Failed to build email: {}", e)))?;

        // Create SMTP transport
        let mailer: AsyncSmtpTransport<Tokio1Executor> = if self.config.use_tls {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.host)
                .map_err(|e: lettre::transport::smtp::Error| EmailError::SmtpConnection(e.to_string()))?
                .port(self.config.port)
                .credentials(Credentials::new(
                    self.config.username.clone(),
                    self.config.password.clone(),
                ))
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.host)
                .map_err(|e: lettre::transport::smtp::Error| EmailError::SmtpConnection(e.to_string()))?
                .port(self.config.port)
                .credentials(Credentials::new(
                    self.config.username.clone(),
                    self.config.password.clone(),
                ))
                .build()
        };

        // Send the email
        let result = mailer.send(email).await;
        result
            .map_err(|e: lettre::transport::smtp::Error| EmailError::Send(format!("Failed to send email: {}", e)))?;

        Ok(())
    }
}
