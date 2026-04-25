use hermes_core::gateway::PlatformAdapter;
use hermes_platform_feishu::FeishuAdapter;
use axum::http::Request;

#[test]
fn test_adapter_name() {
    let adapter = FeishuAdapter::new();
    assert_eq!(adapter.platform_id(), "feishu");
}

#[test]
fn test_adapter_with_credentials() {
    let adapter = FeishuAdapter::new()
        .with_credentials("test_app_id".to_string(), "test_secret".to_string());
    assert_eq!(adapter.platform_id(), "feishu");
}

#[test]
fn test_verify_webhook() {
    let adapter = FeishuAdapter::new();
    let request = Request::builder()
        .uri("/webhook/feishu")
        .body(axum::body::Body::empty())
        .unwrap();

    // 当前实现总是返回 true（TODO: 实现完整签名验证）
    assert!(adapter.verify_webhook(&request));
}

#[tokio::test]
async fn test_parse_inbound_valid_message() {
    use serde_json::json;

    let adapter = FeishuAdapter::new();

    let event = json!({
        "schema": "2.0",
        "header": {
            "event_id": "evt_12345",
            "event_type": "im.message.receive_v1",
            "create_time": "1700000000000",
            "token": "test_token",
            "app_id": "cli_12345",
            "tenant_key": "test_tenant"
        },
        "event": {
            "sender": {
                "sender_id": {
                    "open_id": "ou_12345",
                    "user_id": "user_12345",
                    "union_id": "union_12345"
                },
                "sender_type": "user",
                "tenant_key": "test_tenant"
            },
            "content": "{\"text\":\"Hello, Feishu!\"}",
            "message_type": "text",
            "create_time": "1700000000000",
            "message_id": "msg_12345",
            "upper_message_id": "",
            "chat_id": "oc_12345",
            "root_id": ""
        }
    });

    let body = serde_json::to_string(&event).unwrap();
    let request = Request::builder()
        .uri("/webhook/feishu")
        .body(axum::body::Body::from(body))
        .unwrap();

    let result: Result<_, hermes_core::gateway::GatewayError> = adapter.parse_inbound(request).await;
    assert!(result.is_ok());

    let msg = result.unwrap();
    assert_eq!(msg.platform, "feishu");
    assert_eq!(msg.sender_id, "ou_12345");
    assert_eq!(msg.content, "Hello, Feishu!");
    assert_eq!(msg.session_id, "feishu:oc_12345");
}

#[tokio::test]
async fn test_parse_inbound_no_event() {
    use serde_json::json;

    let adapter = FeishuAdapter::new();

    let event = json!({
        "schema": "2.0",
        "header": {
            "event_id": "evt_12345",
            "event_type": "im.message.receive_v1"
        }
    });

    let body = serde_json::to_string(&event).unwrap();
    let request = Request::builder()
        .uri("/webhook/feishu")
        .body(axum::body::Body::from(body))
        .unwrap();

    let result: Result<_, hermes_core::gateway::GatewayError> = adapter.parse_inbound(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_parse_inbound_with_user_id() {
    use serde_json::json;

    let adapter = FeishuAdapter::new();

    let event = json!({
        "schema": "2.0",
        "header": {
            "event_id": "evt_12345",
            "event_type": "im.message.receive_v1"
        },
        "event": {
            "sender": {
                "sender_id": {
                    "open_id": "",
                    "user_id": "user_12345",
                    "union_id": ""
                },
                "sender_type": "user"
            },
            "content": "{\"text\":\"Test message\"}",
            "message_type": "text",
            "chat_id": "oc_54321"
        }
    });

    let body = serde_json::to_string(&event).unwrap();
    let request = Request::builder()
        .uri("/webhook/feishu")
        .body(axum::body::Body::from(body))
        .unwrap();

    let result: Result<_, hermes_core::gateway::GatewayError> = adapter.parse_inbound(request).await;
    assert!(result.is_ok());

    let msg = result.unwrap();
    // 应使用 user_id 因为 open_id 为空
    assert_eq!(msg.sender_id, "user_12345");
}
