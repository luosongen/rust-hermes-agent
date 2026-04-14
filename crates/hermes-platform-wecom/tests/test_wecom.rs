use hermes_core::gateway::PlatformAdapter;
use hermes_platform_wecom::WeComAdapter;
use axum::http::Request;

#[test]
fn test_verify_webhook_with_required_params() {
    let adapter = WeComAdapter::new(
        "corp_id".into(),
        "agent_id".into(),
        "test_token".into(),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
    );

    let request = Request::builder()
        .uri("/webhook/wecom?msg_signature=abc&timestamp=123&nonce=xyz")
        .body(axum::body::Body::empty())
        .unwrap();

    assert!(adapter.verify_webhook(&request));
}

#[test]
fn test_verify_webhook_missing_params() {
    let adapter = WeComAdapter::new(
        "corp_id".into(),
        "agent_id".into(),
        "test_token".into(),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
    );

    let request = Request::builder()
        .uri("/webhook/wecom?msg_signature=abc")
        .body(axum::body::Body::empty())
        .unwrap();

    assert!(!adapter.verify_webhook(&request));
}

#[tokio::test]
async fn test_parse_inbound_invalid_xml() {
    let adapter = WeComAdapter::new(
        "corp_id".into(),
        "agent_id".into(),
        "test_token".into(),
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
    );

    let body = r#"<xml><not><the><right><format>"#;
    let request = Request::builder()
        .uri("/webhook/wecom?msg_signature=a&timestamp=b&nonce=c")
        .body(axum::body::Body::from(body))
        .unwrap();

    let result: Result<_, hermes_core::gateway::GatewayError> = adapter.parse_inbound(request).await;
    assert!(result.is_err());
}
