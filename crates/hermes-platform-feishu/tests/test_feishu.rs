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
        .header("X-Feishu-Encryption-Key", "test-encryption-key")
        .body(axum::body::Body::empty())
        .unwrap();

    // Feishu webhook 需要加密密钥头部
    assert!(adapter.verify_webhook(&request));
}

#[test]
fn test_verify_webhook_without_header() {
    let adapter = FeishuAdapter::new();
    let request = Request::builder()
        .uri("/webhook/feishu")
        .body(axum::body::Body::empty())
        .unwrap();

    // 没有加密密钥头部时返回 false
    assert!(!adapter.verify_webhook(&request));
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

#[tokio::test]
async fn test_parse_inbound_image_message() {
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
                    "open_id": "ou_12345"
                },
                "sender_type": "user"
            },
            "content": "{\"image_key\":\"img_abc123\"}",
            "message_type": "image",
            "chat_id": "oc_12345"
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
    assert_eq!(msg.content, "[图片] img_abc123");
}

#[tokio::test]
async fn test_parse_inbound_audio_message() {
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
                    "open_id": "ou_12345"
                },
                "sender_type": "user"
            },
            "content": "{\"audio_key\":\"audio_xyz789\"}",
            "message_type": "audio",
            "chat_id": "oc_12345"
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
    assert_eq!(msg.content, "[音频] audio_xyz789");
}

#[tokio::test]
async fn test_parse_inbound_video_message() {
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
                    "open_id": "ou_12345"
                },
                "sender_type": "user"
            },
            "content": "{\"video_key\":\"video_def456\"}",
            "message_type": "video",
            "chat_id": "oc_12345"
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
    assert_eq!(msg.content, "[视频] video_def456");
}

#[tokio::test]
async fn test_parse_inbound_file_message() {
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
                    "open_id": "ou_12345"
                },
                "sender_type": "user"
            },
            "content": "{\"file_key\":\"file_ghi789\"}",
            "message_type": "file",
            "chat_id": "oc_12345"
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
    assert_eq!(msg.content, "[文件] file_ghi789");
}

#[tokio::test]
async fn test_parse_inbound_post_message() {
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
                    "open_id": "ou_12345"
                },
                "sender_type": "user"
            },
            "content": "{\"text\":\"Hello from post!\"}",
            "message_type": "post",
            "chat_id": "oc_12345"
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
    assert_eq!(msg.content, "Hello from post!");
}
