use hermes_core::gateway::PlatformAdapter;
use hermes_platform_telegram::TelegramAdapter;
use axum::http::Request;

#[test]
fn test_verify_webhook_with_valid_token() {
    let adapter = TelegramAdapter::new(
        "test_bot_token".into(),
        "secret_token".into(),
    );

    let request = Request::builder()
        .uri("/webhook/telegram?secret_token=secret_token")
        .body(axum::body::Body::empty())
        .unwrap();

    assert!(adapter.verify_webhook(&request));
}

#[test]
fn test_verify_webhook_with_invalid_token() {
    let adapter = TelegramAdapter::new(
        "test_bot_token".into(),
        "secret_token".into(),
    );

    let request = Request::builder()
        .uri("/webhook/telegram?secret_token=wrong_token")
        .body(axum::body::Body::empty())
        .unwrap();

    assert!(!adapter.verify_webhook(&request));
}

#[test]
fn test_verify_webhook_with_no_token() {
    let adapter = TelegramAdapter::new(
        "test_bot_token".into(),
        "secret_token".into(),
    );

    let request = Request::builder()
        .uri("/webhook/telegram")
        .body(axum::body::Body::empty())
        .unwrap();

    assert!(!adapter.verify_webhook(&request));
}

#[tokio::test]
async fn test_parse_inbound_valid_message() {
    use serde_json::json;

    let adapter = TelegramAdapter::new(
        "test_bot_token".into(),
        "secret_token".into(),
    );

    let update = json!({
        "update_id": 12345,
        "message": {
            "chat": { "id": 98765 },
            "text": "Hello, bot!",
            "date": 1700000000
        }
    });

    let body = serde_json::to_string(&update).unwrap();
    let request = Request::builder()
        .uri("/webhook/telegram?secret_token=secret_token")
        .body(axum::body::Body::from(body))
        .unwrap();

    let result: Result<_, hermes_core::gateway::GatewayError> = adapter.parse_inbound(request).await;
    assert!(result.is_ok());

    let msg = result.unwrap();
    assert_eq!(msg.platform, "telegram");
    assert_eq!(msg.sender_id, "98765");
    assert_eq!(msg.content, "Hello, bot!");
    assert_eq!(msg.session_id, "telegram:98765");
}

#[tokio::test]
async fn test_parse_inbound_no_message() {
    use serde_json::json;

    let adapter = TelegramAdapter::new(
        "test_bot_token".into(),
        "secret_token".into(),
    );

    let update = json!({
        "update_id": 12345
    });

    let body = serde_json::to_string(&update).unwrap();
    let request = Request::builder()
        .uri("/webhook/telegram?secret_token=secret_token")
        .body(axum::body::Body::from(body))
        .unwrap();

    let result: Result<_, hermes_core::gateway::GatewayError> = adapter.parse_inbound(request).await;
    assert!(result.is_err());
}
