# Email (SMTP) Platform Adapter 设计文档

## 概述

在 `hermes-platform-email` crate 中实现 Email 平台适配器，支持：
- **入站**：Webhook（第三方邮件 API）+ IMAP 轮询
- **出站**：SMTP 直接发送

## 目标

- Agent 可以接收邮件并响应
- 支持多种入站方式（Webhook/IMAP）
- 通过 SMTP 发送邮件回复

## 架构

```
hermes-platform-email/
├── src/
│   ├── lib.rs              # 模块导出和 EmailAdapter
│   ├── smtp.rs             # SMTP 出站发送
│   ├── imap.rs             # IMAP 轮询入站
│   ├── webhook.rs          # Webhook 入站（第三方 API）
│   ├── parser.rs           # 邮件解析（From/To/Subject/Body）
│   └── error.rs            # EmailError
```

## 核心类型

### EmailAdapter

```rust
pub struct EmailAdapter {
    smtp_config: SmtpConfig,
    imap_config: Option<ImapConfig>,
    webhook_config: Option<WebhookConfig>,
    smtp_client: Arc<RwLock<Option<SmtpClient>>>,
}

impl EmailAdapter {
    pub fn new() -> Self;
    pub fn with_smtp(self, config: SmtpConfig) -> Self;
    pub fn with_imap(self, config: ImapConfig) -> Self;
    pub fn with_webhook(self, config: WebhookConfig) -> Self;
}
```

### 配置结构

```rust
// SMTP 配置
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub use_tls: bool,
}

// IMAP 配置
pub struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval_secs: u64,
}

// Webhook 配置
pub struct WebhookConfig {
    pub secret: String,
    pub providers: Vec<WebhookProvider>, // SendGrid, Mailgun, SES
}
```

## PlatformAdapter 实现

### trait 实现

```rust
#[async_trait]
impl PlatformAdapter for EmailAdapter {
    fn platform_id(&self) -> &'static str { "email" }

    fn verify_webhook(&self, request: &Request) -> bool {
        // 验证 Webhook 签名（支持 SendGrid、Mailgun、SES）
    }

    async fn parse_inbound(&self, request: Request) -> Result<InboundMessage, GatewayError> {
        // 解析邮件内容 → InboundMessage
        // session_id = email:<from_address>
        // sender_id = <from>
        // content = 邮件正文
    }

    async fn send_response(&self, response: ConversationResponse, message: &InboundMessage) -> Result<(), GatewayError> {
        // 通过 SMTP 发送邮件回复给发件人
    }
}
```

### InboundMessage 映射

| 邮件字段 | InboundMessage 字段 |
|----------|-------------------|
| From | sender_id |
| To | session_id (提取邮箱前缀) |
| Subject | 放入 raw metadata |
| Body (text/plain优先) | content |

## Webhook 签名验证

支持三种 Provider：

### SendGrid
```rust
// 验证 X-Twilio-Email-Event-Webhook-Signature header
// 使用 SHA256 HMAC + ECDSA
```

### Mailgun
```rust
// 验证 X-Mailgun-Signature header
// 使用 HMAC SHA256
```

### AWS SES
```rust
// 验证 SHA256 signature
// 使用 receipt rule 配置的 signing secret
```

## IMAP 轮询

```rust
pub struct ImapPoller {
    config: ImapConfig,
    client: Arc<RwLock<Option<ImapClient>>>,
}

impl ImapPoller {
    pub async fn poll(&self) -> Result<Vec<Email>, EmailError>;
    pub async fn mark_read(&self, uid: u32) -> Result<(), EmailError>;
}
```

轮询逻辑：
1. 连接到 IMAP 服务器
2. 搜索 UNSEEN 邮件
3. 获取邮件详情
4. 标记为已读（避免重复处理）

## SMTP 发送

```rust
pub struct SmtpClient {
    config: SmtpConfig,
}

impl SmtpClient {
    pub async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError>;
}
```

发送逻辑：
1. 连接 SMTP 服务器（TLS/STARTTLS）
2. 认证
3. 发送 MAIL FROM / RCPT TO / DATA
4. 断开连接

## 错误类型

```rust
pub enum EmailError {
    SmtpConnection(String),
    SmtpAuth(String),
    ImapConnection(String),
    ImapAuth(String),
    ParseError(String),
    WebhookVerificationFailed,
    NotAuthenticated,
}
```

## 配置示例

```toml
[email]
enabled = true

[email.smtp]
host = "smtp.example.com"
port = 587
username = "noreply@example.com"
password = "smtp-password"
from_address = "Agent <noreply@example.com>"
use_tls = true

[email.imap]
enabled = false
host = "imap.example.com"
port = 993
username = "agent@example.com"
password = "imap-password"
poll_interval_secs = 60

[email.webhook]
enabled = true
secret = "webhook-secret"

[email.webhook.providers]
# 支持多个 provider
- sendgrid
- mailgun
- ses
```

## 与现有模块集成

- `hermes-gateway`：注册 `/webhook/email` 路由
- `hermes-core`：通过 `Agent.run_conversation()` 处理邮件

## 实现顺序

1. `Cargo.toml` + `lib.rs` 基础结构
2. `error.rs` 错误类型
3. `smtp.rs` SMTP 客户端
4. `webhook.rs` Webhook 解析和验证
5. `parser.rs` 邮件解析
6. `imap.rs` IMAP 轮询
7. `lib.rs` 集成 PlatformAdapter
8. 测试
