use hermes_core::gateway::PlatformAdapter;
use hermes_platform_email::{EmailAdapter, ImapConfig, SmtpConfig, WebhookConfig, WebhookProvider};
use axum::body::Body;
use axum::http::Request;

#[tokio::test]
async fn test_adapter_platform_id() {
    let adapter = EmailAdapter::new();
    assert_eq!(adapter.platform_id(), "email");
}

#[tokio::test]
async fn test_adapter_with_webhook_config() {
    let adapter = EmailAdapter::new().with_webhook(WebhookConfig {
        secret: "test-secret".to_string(),
        providers: vec![WebhookProvider::SendGrid],
    });
    assert_eq!(adapter.platform_id(), "email");
}

#[tokio::test]
async fn test_adapter_with_smtp_config() {
    let smtp_config = SmtpConfig {
        host: "smtp.example.com".to_string(),
        port: 587,
        username: "user".to_string(),
        password: "pass".to_string(),
        from_address: "from@example.com".to_string(),
        use_tls: true,
    };
    let adapter = EmailAdapter::new().with_smtp(smtp_config);
    assert_eq!(adapter.platform_id(), "email");
}

#[tokio::test]
async fn test_adapter_with_imap_config() {
    let imap_config = ImapConfig {
        host: "imap.example.com".to_string(),
        port: 993,
        username: "user".to_string(),
        password: "pass".to_string(),
        poll_interval_secs: 60,
        mailbox: Some("INBOX".to_string()),
    };
    let adapter = EmailAdapter::new().with_imap(imap_config);
    assert_eq!(adapter.platform_id(), "email");
}

#[tokio::test]
async fn test_verify_webhook_without_config() {
    let request = Request::builder()
        .body(Body::empty())
        .unwrap();

    let adapter = EmailAdapter::new();
    // Without webhook config set, verify_webhook checks headers
    // Since no relevant headers are set, it should return false
    assert_eq!(adapter.verify_webhook(&request), false);
}

#[tokio::test]
async fn test_verify_webhook_with_sendgrid_header() {
    let request = Request::builder()
        .header("X-Twilio-Email-Event-Webhook-Signature", "test-signature")
        .body(Body::empty())
        .unwrap();

    let adapter = EmailAdapter::new();
    // SendGrid header present, should return true
    assert_eq!(adapter.verify_webhook(&request), true);
}

#[tokio::test]
async fn test_verify_webhook_with_mailgun_header() {
    let request = Request::builder()
        .header("Mailgun-Events-Signature", "test-signature")
        .body(Body::empty())
        .unwrap();

    let adapter = EmailAdapter::new();
    // Mailgun header present, should return true
    assert_eq!(adapter.verify_webhook(&request), true);
}

#[tokio::test]
async fn test_verify_webhook_with_ses_header() {
    let request = Request::builder()
        .header("X-Ses-Sns-Subscription-Arn", "arn:aws:ses:us-east-1:123456789:identity/example.com")
        .body(Body::empty())
        .unwrap();

    let adapter = EmailAdapter::new();
    // SES header present, should return true
    assert_eq!(adapter.verify_webhook(&request), true);
}

#[tokio::test]
async fn test_smtp_config_creation() {
    let smtp_config = SmtpConfig {
        host: "smtp.gmail.com".to_string(),
        port: 465,
        username: "testuser@gmail.com".to_string(),
        password: "testpassword".to_string(),
        from_address: "testuser@gmail.com".to_string(),
        use_tls: true,
    };

    assert_eq!(smtp_config.host, "smtp.gmail.com");
    assert_eq!(smtp_config.port, 465);
    assert_eq!(smtp_config.username, "testuser@gmail.com");
    assert_eq!(smtp_config.password, "testpassword");
    assert_eq!(smtp_config.from_address, "testuser@gmail.com");
    assert_eq!(smtp_config.use_tls, true);
}

#[tokio::test]
async fn test_imap_config_creation() {
    let imap_config = ImapConfig {
        host: "imap.gmail.com".to_string(),
        port: 993,
        username: "testuser@gmail.com".to_string(),
        password: "testpassword".to_string(),
        poll_interval_secs: 300,
        mailbox: Some("INBOX".to_string()),
    };

    assert_eq!(imap_config.host, "imap.gmail.com");
    assert_eq!(imap_config.port, 993);
    assert_eq!(imap_config.username, "testuser@gmail.com");
    assert_eq!(imap_config.password, "testpassword");
    assert_eq!(imap_config.poll_interval_secs, 300);
    assert_eq!(imap_config.mailbox, Some("INBOX".to_string()));
}

#[tokio::test]
async fn test_imap_config_without_mailbox() {
    let imap_config = ImapConfig {
        host: "imap.example.com".to_string(),
        port: 993,
        username: "user".to_string(),
        password: "pass".to_string(),
        poll_interval_secs: 60,
        mailbox: None,
    };

    assert_eq!(imap_config.mailbox, None);
}

#[tokio::test]
async fn test_webhook_provider_sendgrid() {
    let provider = WebhookProvider::SendGrid;
    assert_eq!(format!("{:?}", provider), "SendGrid");
}

#[tokio::test]
async fn test_webhook_provider_mailgun() {
    let provider = WebhookProvider::Mailgun;
    assert_eq!(format!("{:?}", provider), "Mailgun");
}

#[tokio::test]
async fn test_webhook_provider_ses() {
    let provider = WebhookProvider::Ses;
    assert_eq!(format!("{:?}", provider), "Ses");
}

#[tokio::test]
async fn test_webhook_config_with_multiple_providers() {
    let config = WebhookConfig {
        secret: "my-secret-key".to_string(),
        providers: vec![WebhookProvider::SendGrid, WebhookProvider::Mailgun, WebhookProvider::Ses],
    };

    assert_eq!(config.secret, "my-secret-key");
    assert_eq!(config.providers.len(), 3);
    assert_eq!(config.providers[0], WebhookProvider::SendGrid);
    assert_eq!(config.providers[1], WebhookProvider::Mailgun);
    assert_eq!(config.providers[2], WebhookProvider::Ses);
}

#[tokio::test]
async fn test_adapter_builder_pattern() {
    let smtp_config = SmtpConfig {
        host: "smtp.example.com".to_string(),
        port: 587,
        username: "user".to_string(),
        password: "pass".to_string(),
        from_address: "from@example.com".to_string(),
        use_tls: true,
    };

    let imap_config = ImapConfig {
        host: "imap.example.com".to_string(),
        port: 993,
        username: "user".to_string(),
        password: "pass".to_string(),
        poll_interval_secs: 60,
        mailbox: Some("INBOX".to_string()),
    };

    let webhook_config = WebhookConfig {
        secret: "webhook-secret".to_string(),
        providers: vec![WebhookProvider::SendGrid],
    };

    let adapter = EmailAdapter::new()
        .with_smtp(smtp_config)
        .with_imap(imap_config)
        .with_webhook(webhook_config);

    assert_eq!(adapter.platform_id(), "email");
}
