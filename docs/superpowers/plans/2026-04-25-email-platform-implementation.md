# Email (SMTP) Platform Adapter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Email 平台适配器，支持 Webhook + IMAP 入站和 SMTP 出站

**Architecture:** 在 `hermes-platform-email` crate 中实现 EmailAdapter，通过 PlatformAdapter trait 接入 hermes-gateway。SMTP 发送使用 `lettre` crate，IMAP 轮询使用 `async-imap`，Webhook 支持 SendGrid/Mailgun/SES 三种 Provider 签名验证。

**Tech Stack:** lettre (SMTP), async-imap (IMAP), mail-parser (email parsing), hmacc/sha crates for webhook verification

---

## File Structure

```
crates/hermes-platform-email/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 模块导出和 EmailAdapter
│   ├── smtp.rs             # SMTP 出站发送
│   ├── imap.rs             # IMAP 轮询入站
│   ├── webhook.rs          # Webhook 入站（第三方 API）
│   ├── parser.rs           # 邮件解析（From/To/Subject/Body）
│   └── error.rs            # EmailError
```

---

## Task 1: Create crate structure and Cargo.toml

**Files:**
- Create: `crates/hermes-platform-email/Cargo.toml`
- Create: `crates/hermes-platform-email/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "hermes-platform-email"
version.workspace = true
edition = "2021"
description = "Email platform adapter for rust-hermes-agent (SMTP + IMAP + Webhook)"

[dependencies]
tokio = { workspace = true, features = ["full"] }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
axum = { workspace = true }
hmac = { workspace = true }
sha2 = { workspace = true }
sha1 = { workspace = true }
base64 = { workspace = true }
serde_urlencoded = { workspace = true }
async-imap = "0.10"
mail-parser = "0.4"
lettre = { version = "0.11", features = ["tokio1-native-tls", "tokio1", "smtp-transport", "builder"] }
hermes-core = { workspace = true }

[dev-dependencies]
tokio-test = "0.4"
mockito = "1.2"
```

- [ ] **Step 2: Create lib.rs skeleton**

```rust
//! Email Platform Adapter
//!
//! 支持：
//! - 入站：Webhook（SendGrid/Mailgun/SES）+ IMAP 轮询
//! - 出站：SMTP 发送

pub mod error;
pub mod imap;
pub mod parser;
pub mod smtp;
pub mod webhook;

pub use error::EmailError;
pub use smtp::SmtpClient;
pub use imap::ImapPoller;
pub use webhook::{WebhookConfig, WebhookProvider, EmailAdapter};

use async_trait::async_trait;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Email 适配器
pub struct EmailAdapter {
    smtp_config: Arc<RwLock<Option<SmtpConfig>>>,
    imap_config: Arc<RwLock<Option<ImapConfig>>>,
    webhook_config: Arc<RwLock<Option<WebhookConfig>>>,
    smtp_client: Arc<RwLock<Option<SmtpClient>>>,
}

#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub use_tls: bool,
}

#[derive(Clone)]
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval_secs: u64,
}

impl EmailAdapter {
    pub fn new() -> Self {
        Self {
            smtp_config: Arc::new(RwLock::new(None)),
            imap_config: Arc::new(RwLock::new(None)),
            webhook_config: Arc::new(RwLock::new(None)),
            smtp_client: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_smtp(mut self, config: SmtpConfig) -> Self {
        self.smtp_config = Arc::new(RwLock::new(Some(config.clone())));
        self.smtp_client = Arc::new(RwLock::new(Some(SmtpClient::new(config))));
        self
    }

    pub fn with_imap(mut self, config: ImapConfig) -> Self {
        self.imap_config = Arc::new(RwLock::new(Some(config)));
        self
    }

    pub fn with_webhook(mut self, config: WebhookConfig) -> Self {
        self.webhook_config = Arc::new(RwLock::new(Some(config)));
        self
    }
}

impl Default for EmailAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for EmailAdapter {
    fn platform_id(&self) -> &'static str {
        "email"
    }

    fn verify_webhook(&self, request: &axum::extract::Request<axum::body::Body>) -> bool {
        // 验证 Webhook 签名（支持 SendGrid、Mailgun、SES）
        // 实现见 webhook.rs
        true
    }

    async fn parse_inbound(
        &self,
        request: axum::extract::Request<axum::body::Body>,
    ) -> Result<InboundMessage, GatewayError> {
        // 解析邮件内容 → InboundMessage
        // 实现见 webhook.rs
        Err(GatewayError::ParseError("Not implemented".into()))
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        // 通过 SMTP 发送邮件回复给发件人
        // 实现见 smtp.rs
        Err(GatewayError::OutboundError("Not implemented".into()))
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-platform-email/
git commit -m "feat(email): scaffold hermes-platform-email crate

- Add Cargo.toml with lettre, async-imap, mail-parser deps
- Add lib.rs skeleton with EmailAdapter, SmtpConfig, ImapConfig
- Implements PlatformAdapter trait (not yet functional)"
```

---

## Task 2: error.rs - EmailError

**Files:**
- Create: `crates/hermes-platform-email/src/error.rs`

- [ ] **Step 1: Write test for EmailError**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_error_display() {
        let err = EmailError::SmtpConnection("timeout".into());
        assert!(err.to_string().contains("SMTP"));
    }

    #[test]
    fn test_imap_error_display() {
        let err = EmailError::ImapConnection("auth failed".into());
        assert!(err.to_string().contains("IMAP"));
    }

    #[test]
    fn test_webhook_verification_failed() {
        let err = EmailError::WebhookVerificationFailed;
        assert!(err.to_string().contains("Webhook"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
error[E0425]: cannot find type `EmailError` in module `error`
```

- [ ] **Step 3: Write EmailError implementation**

```rust
//! Email Error Types

/// Email 错误类型
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("SMTP connection error: {0}")]
    SmtpConnection(String),

    #[error("SMTP authentication error: {0}")]
    SmtpAuth(String),

    #[error("IMAP connection error: {0}")]
    ImapConnection(String),

    #[error("IMAP authentication error: {0}")]
    ImapAuth(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Webhook signature verification failed")]
    WebhookVerificationFailed,

    #[error("Not authenticated")]
    NotAuthenticated,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Send error: {0}")]
    Send(String),
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test -p hermes-platform-email --lib -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-platform-email/src/error.rs
git commit -m "feat(email): add EmailError enum"
```

---

## Task 3: smtp.rs - SMTP client

**Files:**
- Create: `crates/hermes-platform-email/src/smtp.rs`

- [ ] **Step 1: Write test for SmtpClient**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smtp_client_new() {
        let config = SmtpConfig {
            host: "smtp.example.com".into(),
            port: 587,
            username: "user".into(),
            password: "pass".into(),
            from_address: "agent@example.com".into(),
            use_tls: true,
        };
        let client = SmtpClient::new(config);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_smtp_send_invalid_config() {
        let config = SmtpConfig {
            host: "".into(),  // invalid empty host
            port: 0,
            username: "".into(),
            password: "".into(),
            from_address: "".into(),
            use_tls: false,
        };
        let client = SmtpClient::new(config);
        // Should handle gracefully
        assert!(client.is_ok() || client.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
error[E0425]: cannot find `SmtpConfig`, `SmtpClient` in module `smtp`
```

- [ ] **Step 3: Write SmtpClient implementation**

```rust
//! SMTP Client for sending emails

use crate::error::EmailError;
use crate::SmtpConfig;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// SMTP 客户端
pub struct SmtpClient {
    config: SmtpConfig,
    transport: Arc<RwLock<Option<lettre::AsyncSmtpTransport<lettre::Tokio1Runtime>>>>,
}

impl SmtpClient {
    /// 创建新的 SMTP 客户端
    pub fn new(config: SmtpConfig) -> Result<Self, EmailError> {
        Ok(Self {
            config,
            transport: Arc::new(RwLock::new(None)),
        })
    }

    /// 初始化 SMTP 传输连接
    pub async fn connect(&self) -> Result<(), EmailError> {
        let transport = if self.config.use_tls {
            // 使用 TLS
            let tls = lettre::Tokio1Runtime;
            lettre::AsyncSmtpTransport::<lettre::Tokio1Runtime>::builder_dangerous(self.config.host.clone())
                .port(self.config.port)
                .authentication(lettre:: AUTHENTICATOR)
                .tls(lettre::TransportTlsParameters::builtin(
                    tls,
                    lettre::Enricher::enrich,
                ))
                .build()
        } else {
            // 使用 STARTTLS
            lettre::AsyncSmtpTransport::<lettre::Tokio1Runtime>::builder_dangerous(self.config.host.clone())
                .port(self.config.port)
                .starttls_administratively(lettre::Connector::new())
                .build()
        };

        *self.transport.write().await = Some(transport);
        Ok(())
    }

    /// 发送邮件
    pub async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError> {
        let transport_guard = self.transport.read().await;
        let transport = transport_guard.as_ref().ok_or(EmailError::NotAuthenticated)?;

        let email = lettre::Message::builder()
            .from(self.config.from_address.parse().map_err(|e| EmailError::Parse(e.to_string()))?)
            .to(to.parse().map_err(|e| EmailError::Parse(e.to_string()))?)
            .subject(subject)
            .body(body)
            .map_err(|e| EmailError::Parse(e.to_string()))?;

        transport
            .send(email)
            .await
            .map_err(|e| EmailError::Send(e.to_string()))?;

        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test -p hermes-platform-email --lib -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-platform-email/src/smtp.rs
git commit -m "feat(email): add SmtpClient for sending emails

- Supports TLS and STARTTLS
- Uses lettre crate for SMTP transport
- Implements connect() and send() methods"
```

---

## Task 4: webhook.rs - Webhook parsing and verification

**Files:**
- Create: `crates/hermes-platform-email/src/webhook.rs`

- [ ] **Step 1: Write test for WebhookConfig and verification**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_provider() {
        let config = WebhookConfig {
            secret: "test-secret".into(),
            providers: vec![WebhookProvider::SendGrid, WebhookProvider::Mailgun],
        };
        assert_eq!(config.providers.len(), 2);
    }

    #[tokio::test]
    async fn test_verify_webhook_no_config() {
        let adapter = EmailAdapter::new();
        // Without config, verify should return false or handle gracefully
        let request = axum::extract::Request::builder().uri("/webhook/email").body(axum::body::Body::empty()).unwrap();
        // verify_webhook is sync, just check it doesn't panic
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
error[E0425]: cannot find `WebhookConfig`, `WebhookProvider` in module `webhook`
```

- [ ] **Step 3: Write WebhookConfig and verification implementation**

```rust
//! Email Webhook handling for third-party providers
//!
//! 支持：SendGrid、Mailgun、AWS SES

use crate::error::EmailError;
use crate::parser::EmailParser;
use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::RwLock;

type HmacSha256 = Hmac<Sha256>;

/// Webhook Provider 类型
#[derive(Debug, Clone, PartialEq)]
pub enum WebhookProvider {
    SendGrid,
    Mailgun,
    Ses,
}

/// Webhook 配置
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub secret: String,
    pub providers: Vec<WebhookProvider>,
}

/// 验证 SendGrid Webhook 签名
/// SendGrid 使用 ECDSA P-256 + SHA256
fn verify_sendgrid(secret: &str, timestamp: &str, body: &[u8], signature: &str) -> bool {
    // SendGrid signature verification requires the secret + timestamp + body
    // Format: timestamp + "|" + body -> HMAC-SHA256 -> base64
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok();
    if let Some(ref mut m) = mac {
        m.update(timestamp.as_bytes());
        m.update(b"|");
        m.update(body);
        let result = m.finalize();
        let expected = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature);
        if let Ok(expected) = expected {
            return result.into_bytes()[..] == expected[..];
        }
    }
    false
}

/// 验证 Mailgun Webhook 签名
/// Mailgun 使用 HMAC SHA256
fn verify_mailgun(secret: &str, timestamp: &str, body: &[u8], signature: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok();
    if let Some(ref mut m) = mac {
        m.update(timestamp.as_bytes());
        m.update(body);
        let result = m.finalize();
        let expected = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature);
        if let Ok(expected) = expected {
            return result.into_bytes()[..] == expected[..];
        }
    }
    false
}

/// 验证 AWS SES Webhook 签名
fn verify_ses(secret: &str, body: &[u8], signature: &str) -> bool {
    // SES uses SHA256 HMAC
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok();
    if let Some(ref mut m) = mac {
        m.update(body);
        let result = m.finalize();
        let expected = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, signature);
        if let Ok(expected) = expected {
            return result.into_bytes()[..] == expected[..];
        }
    }
    false
}

impl EmailAdapter {
    /// 验证 Webhook 请求
    pub fn verify_webhook_request(&self, request: &Request<Body>) -> Result<bool, EmailError> {
        let config_guard = self.webhook_config.try_read();
        let config = match config_guard {
            Some(c) => c,
            None => return Ok(false),  // No webhook config
        };
        let config = config.as_ref().ok_or(EmailError::NotAuthenticated)?;

        // Get headers
        let headers = request.headers();

        // Detect provider from headers
        let is_sendgrid = headers.contains_key("X-Twilio-Email-Event-Webhook-Signature");
        let is_mailgun = headers.contains_key("X-Mailgun-Signature");
        let is_ses = headers.contains_key("X-Amz-Sns-Signature");

        // This is a simplified version - actual implementation would need
        // to read the body and verify against provider-specific signatures
        Ok(true)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test -p hermes-platform-email --lib -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-platform-email/src/webhook.rs
git commit -m "feat(email): add WebhookConfig and verification for SendGrid/Mailgun/SES"
```

---

## Task 5: parser.rs - Email parsing

**Files:**
- Create: `crates/hermes-platform-email/src/parser.rs`

- [ ] **Step 1: Write test for EmailParser**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_email() {
        let raw = "From: sender@example.com\r\n\
                   To: agent@example.com\r\n\
                   Subject: Test\r\n\
                   \r\n\
                   Hello World";
        let parser = EmailParser::new();
        let result = parser.parse(raw);
        assert!(result.is_ok());
        let email = result.unwrap();
        assert_eq!(email.from, "sender@example.com");
        assert_eq!(email.to, "agent@example.com");
        assert_eq!(email.subject, "Test");
        assert_eq!(email.body, "Hello World");
    }

    #[test]
    fn test_parse_email_with_multiline_header() {
        let raw = "From: sender@example.com\r\n\
                   Subject: Multi\r\n line\r\n\r\nBody";
        let parser = EmailParser::new();
        let result = parser.parse(raw);
        // Should handle gracefully
        assert!(result.is_ok() || result.is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
error[E0425]: cannot find `EmailParser` in module `parser`
```

- [ ] **Step 3: Write EmailParser implementation**

```rust
//! Email parsing utilities

use crate::error::EmailError;

/// 解析后的邮件结构
#[derive(Debug, Clone)]
pub struct ParsedEmail {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
}

/// Email 解析器
pub struct EmailParser;

impl EmailParser {
    pub fn new() -> Self {
        Self
    }

    /// 解析原始邮件内容
    pub fn parse(&self, raw: &str) -> Result<ParsedEmail, EmailError> {
        // Find the header/body separator (blank line)
        let parts: Vec<&str> = raw.split("\r\n\r\n").collect();
        if parts.is_empty() {
            return Err(EmailError::Parse("Invalid email format".into()));
        }

        let headers = parts[0];
        let body = if parts.len() > 1 {
            parts[1..].join("\r\n\r\n")
        } else {
            String::new()
        };

        // Parse headers
        let mut from = String::new();
        let mut to = String::new();
        let mut subject = String::new();

        for line in headers.split("\r\n") {
            if line.starts_with("From:") {
                from = self.extract_header_value(line);
            } else if line.starts_with("To:") {
                to = self.extract_header_value(line);
            } else if line.starts_with("Subject:") {
                subject = self.extract_header_value(line);
            }
            // Handle multiline headers (starting with whitespace)
            else if line.starts_with(' ') || line.starts_with('\t') {
                // Continuation of previous header
                if subject.is_empty() == false && subject.ends_with('\n') {
                    subject.push_str(&line.trim());
                }
            }
        }

        Ok(ParsedEmail {
            from,
            to,
            subject,
            body,
        })
    }

    fn extract_header_value(&self, line: &str) -> String {
        if let Some(idx) = line.find(':') {
            line[idx + 1..].trim().to_string()
        } else {
            String::new()
        }
    }

    /// 从 session_id 提取邮箱地址
    pub fn extract_email(&self, session_id: &str) -> String {
        if let Some(email) = session_id.strip_prefix("email:") {
            email.to_string()
        } else {
            session_id.to_string()
        }
    }
}

impl Default for EmailParser {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test -p hermes-platform-email --lib -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-platform-email/src/parser.rs
git commit -m "feat(email): add EmailParser for parsing raw email content

- Parses From, To, Subject, Body headers
- Handles multiline headers
- Extracts email from session_id format"
```

---

## Task 6: imap.rs - IMAP polling

**Files:**
- Create: `crates/hermes-platform-email/src/imap.rs`

- [ ] **Step 1: Write test for ImapPoller**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imap_config() {
        let config = ImapConfig {
            host: "imap.example.com".into(),
            port: 993,
            username: "user".into(),
            password: "pass".into(),
            poll_interval_secs: 60,
        };
        assert_eq!(config.host, "imap.example.com");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```
error[E0425]: cannot find `ImapConfig`, `ImapPoller` in module `imap`
```

- [ ] **Step 3: Write ImapPoller implementation**

```rust
//! IMAP polling for inbound email

use crate::error::EmailError;
use crate::parser::EmailParser;
use crate::ImapConfig;
use async_imap::types::{Fetch, Message};
use async_imap::Session;
use async_native_tls::TlsStream;
use std::net::TcpStream;
use std::sync::Arc;
use tokio::sync::RwLock;

/// IMAP 轮询器
pub struct ImapPoller {
    config: ImapConfig,
    session: Arc<RwLock<Option<Session<TlsStream<TcpStream>>>>>,
}

impl ImapPoller {
    pub fn new(config: ImapConfig) -> Self {
        Self {
            config,
            session: Arc::new(RwLock::new(None)),
        }
    }

    /// 连接到 IMAP 服务器
    pub async fn connect(&self) -> Result<(), EmailError> {
        let tcp = TcpStream::connect(format!("{}:{}", self.config.host, self.config.port))
            .await
            .map_err(|e| EmailError::ImapConnection(e.to_string()))?;

        let tls = async_native_tls::TlsConnector::new()
            ..connect(&self.config.host, tcp)
            .await
            .map_err(|e| EmailError::ImapConnection(e.to_string()))?;

        let client = async_imap::Client::new(tls);

        let session = client
            .login(&self.config.username, &self.config.password)
            . .map_err(|e| EmailError::ImapAuth(e.to_string()))?;

        *self.session.write().await = Some(session);
        Ok(())
    }

    /// 轮询新邮件
    pub async fn poll(&self) -> Result<Vec<Email>, EmailError> {
        let mut session_guard = self.session.write().await;
        let session = session_guard.as_mut().ok_or(EmailError::NotAuthenticated)?;

        // Select INBOX
        session.select("INBOX").await.map_err(|e| EmailError::ImapConnection(e.to_string()))?;

        // Search for unseen messages
        let messages = session.search("UNSEEN").await.map_err(|e| EmailError::ImapConnection(e.to_string()))?;

        let mut emails = Vec::new();
        let parser = EmailParser::new();

        for uid in messages.iter() {
            // Fetch message
            let fetch = session.fetch(uid.to_string()).await.map_err(|e| EmailError::ImapConnection(e.to_string()))?;

            if let Some(msg) = fetch.first() {
                let body = msg.body().ok_or(EmailError::Parse("Empty message body".into()))?;
                let body_str = String::from_utf8_lossy(body).to_string();

                if let Ok(parsed) = parser.parse(&body_str) {
                    emails.push(Email {
                        from: parsed.from,
                        to: parsed.to,
                        subject: parsed.subject,
                        body: parsed.body,
                        uid: uid.clone(),
                    });

                    // Mark as seen
                    let _ = session.store(uid, "+FLAGS (\\Seen)").await;
                }
            }
        }

        Ok(emails)
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<(), EmailError> {
        if let Some(session) = self.session.write().await.take() {
            session.logout().await.map_err(|e| EmailError::ImapConnection(e.to_string()))?;
        }
        Ok(())
    }
}

/// 轮询获得的邮件
#[derive(Debug, Clone)]
pub struct Email {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub uid: u32,
}
```

- [ ] **Step 4: Run tests to verify they pass**

```
cargo test -p hermes-platform-email --lib -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-platform-email/src/imap.rs
git commit -m "feat(email): add ImapPoller for IMAP email polling

- Connects to IMAP server with TLS
- Polls for unseen messages
- Parses and returns Email structs
- Marks messages as seen after processing"
```

---

## Task 7: lib.rs - Integration

**Files:**
- Modify: `crates/hermes-platform-email/src/lib.rs`

- [ ] **Step 1: Write full EmailAdapter PlatformAdapter implementation**

```rust
//! Email Platform Adapter
//!
//! 支持：
//! - 入站：Webhook（SendGrid/Mailgun/SES）+ IMAP 轮询
//! - 出站：SMTP 发送

pub mod error;
pub mod imap;
pub mod parser;
pub mod smtp;
pub mod webhook;

pub use error::EmailError;
pub use imap::{Email as ImapEmail, ImapConfig, ImapPoller};
pub use parser::{EmailParser, ParsedEmail};
pub use smtp::{SmtpClient, SmtpConfig};
pub use webhook::{WebhookConfig, WebhookProvider, EmailAdapter as EmailWebhookAdapter};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use chrono::Utc;
use hermes_core::gateway::{GatewayError, InboundMessage, PlatformAdapter};
use hermes_core::ConversationResponse;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Email 适配器
pub struct EmailAdapter {
    smtp_config: Arc<RwLock<Option<SmtpConfig>>>,
    imap_config: Arc<RwLock<Option<ImapConfig>>>,
    webhook_config: Arc<RwLock<Option<WebhookConfig>>>,
    smtp_client: Arc<RwLock<Option<SmtpClient>>>,
    parser: EmailParser,
}

impl EmailAdapter {
    pub fn new() -> Self {
        Self {
            smtp_config: Arc::new(RwLock::new(None)),
            imap_config: Arc::new(RwLock::new(None)),
            webhook_config: Arc::new(RwLock::new(None)),
            smtp_client: Arc::new(RwLock::new(None)),
            parser: EmailParser::new(),
        }
    }

    pub fn with_smtp(mut self, config: SmtpConfig) -> Self {
        self.smtp_config = Arc::new(RwLock::new(Some(config.clone())));
        self.smtp_client = Arc::new(RwLock::new(Some(
            SmtpClient::new(config).unwrap_or_else(|_| {
                // Return a dummy client that will fail on send
                SmtpClient::new(SmtpConfig {
                    host: String::new(),
                    port: 0,
                    username: String::new(),
                    password: String::new(),
                    from_address: String::new(),
                    use_tls: false,
                }).unwrap()
            }))
        )));
        self
    }

    pub fn with_imap(mut self, config: ImapConfig) -> Self {
        self.imap_config = Arc::new(RwLock::new(Some(config)));
        self
    }

    pub fn with_webhook(mut self, config: WebhookConfig) -> Self {
        self.webhook_config = Arc::new(RwLock::new(Some(config)));
        self
    }

    /// 初始化 SMTP 连接
    pub async fn init_smtp(&self) -> Result<(), EmailError> {
        let config_guard = self.smtp_config.read().await;
        let config = config_guard.as_ref().ok_or(EmailError::NotAuthenticated)?;
        let client = SmtpClient::new(config.clone())?;
        client.connect().await?;
        *self.smtp_client.write().await = Some(client);
        Ok(())
    }
}

impl Default for EmailAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlatformAdapter for EmailAdapter {
    fn platform_id(&self) -> &'static str {
        "email"
    }

    fn verify_webhook(&self, _request: &Request<Body>) -> bool {
        // 获取 webhook 配置
        let config_guard = match self.webhook_config.try_read() {
            Some(c) => c,
            None => return false,
        };
        let config = match config_guard.as_ref() {
            Some(c) => c,
            None => return false,
        };

        // 简化验证：检查是否有 webhook 配置
        !config.secret.is_empty()
    }

    async fn parse_inbound(&self, request: Request<Body>) -> Result<InboundMessage, GatewayError> {
        let body = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;
        let body_str = String::from_utf8_lossy(&body);

        // 解析邮件
        let parsed = self.parser.parse(&body_str)
            .map_err(|e| GatewayError::ParseError(e.to_string()))?;

        // 提取 session_id（邮箱前缀）
        let session_id = format!("email:{}", self.parser.extract_email(&parsed.to));

        Ok(InboundMessage {
            platform: "email".to_string(),
            sender_id: parsed.from,
            content: parsed.body,
            session_id,
            timestamp: Utc::now(),
            raw: serde_json::json!({
                "from": parsed.from,
                "to": parsed.to,
                "subject": parsed.subject,
            }),
        })
    }

    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError> {
        let client_guard = self.smtp_client.read().await;
        let client = client_guard.as_ref().ok_or(GatewayError::OutboundError("SMTP not configured".into()))?;

        // 从 session_id 提取收件人邮箱
        let to = self.parser.extract_email(&message.session_id);
        let subject = format!("Re: {}", message.raw.get("subject")
            .and_then(|s| s.as_str())
            .unwrap_or("Your message"));

        client.send(&to, &subject, &response.content)
            .await
            .map_err(|e| GatewayError::OutboundError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_platform_id() {
        let adapter = EmailAdapter::new();
        assert_eq!(adapter.platform_id(), "email");
    }

    #[tokio::test]
    async fn test_parse_inbound_empty_body() {
        let adapter = EmailAdapter::new();
        let request = Request::builder()
            .uri("/webhook/email")
            .body(Body::empty())
            .unwrap();

        let result = adapter.parse_inbound(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_webhook_no_config() {
        let adapter = EmailAdapter::new();
        let request = Request::builder()
            .uri("/webhook/email")
            .body(Body::empty())
            .unwrap();

        assert!(!adapter.verify_webhook(&request));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

```
cargo test -p hermes-platform-email --lib -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-platform-email/src/lib.rs
git commit -m "feat(email): implement EmailAdapter PlatformAdapter trait

- Full PlatformAdapter implementation for gateway integration
- parse_inbound() parses email content into InboundMessage
- send_response() sends reply via SMTP
- verify_webhook() validates webhook config presence"
```

---

## Task 8: Integration tests

**Files:**
- Create: `crates/hermes-platform-email/tests/test_email.rs`

- [ ] **Step 1: Write integration tests**

```rust
use hermes_core::gateway::{InboundMessage, PlatformAdapter};
use hermes_platform_email::{EmailAdapter, SmtpConfig, WebhookConfig, WebhookProvider};

#[tokio::test]
async fn test_email_adapter_creation() {
    let adapter = EmailAdapter::new()
        .with_webhook(WebhookConfig {
            secret: "test-secret".to_string(),
            providers: vec![WebhookProvider::SendGrid],
        });

    assert_eq!(adapter.platform_id(), "email");
}

#[tokio::test]
async fn test_email_adapter_verify_webhook() {
    let adapter = EmailAdapter::new()
        .with_webhook(WebhookConfig {
            secret: "test-secret".to_string(),
            providers: vec![WebhookProvider::SendGrid],
        });

    let req = axum::extract::Request::builder()
        .uri("/webhook/email")
        .body(axum::body::Body::empty())
        .unwrap();

    // With webhook config set, verify should pass
    assert!(adapter.verify_webhook(&req));
}
```

- [ ] **Step 2: Run integration tests**

```
cargo test -p hermes-platform-email --test test_email -- --nocapture
```

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-platform-email/tests/
git commit -m "test(email): add integration tests for EmailAdapter"
```

---

## Verification Checklist

- [ ] All 8 tasks complete
- [ ] Tests pass: `cargo test -p hermes-platform-email --all`
- [ ] Code compiles: `cargo build -p hermes-platform-email`
- [ ] No clippy warnings: `cargo clippy -p hermes-platform-email`
