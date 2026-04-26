//! Gateway Webhook Integration Tests

use axum::body::Body;
use axum::http::Request;
use hermes_core::gateway::PlatformAdapter;

/// 测试各个平台 webhook 路由存在
#[test]
fn test_adapter_platform_ids() {
    use hermes_platform_telegram::TelegramAdapter;
    use hermes_platform_wecom::WeComAdapter;
    use hermes_platform_dingtalk::DingTalkAdapter;
    use hermes_platform_feishu::FeishuAdapter;
    use hermes_platform_sms::SmsAdapter;

    // WeCom 需要有效的 base64 编码的 AES key
    let wecom_aes = "0123456789abcdef0123456789abcdef".to_string(); // 32 bytes
    assert_eq!(TelegramAdapter::new("bot".to_string(), "verify".to_string()).platform_id(), "telegram");
    assert_eq!(WeComAdapter::new("corp".to_string(), "agent".to_string(), "token".to_string(), wecom_aes).platform_id(), "wecom");
    assert_eq!(DingTalkAdapter::new().platform_id(), "dingtalk");
    assert_eq!(FeishuAdapter::new().platform_id(), "feishu");
    assert_eq!(SmsAdapter::new().platform_id(), "sms");
}

/// 测试 Telegram 适配器解析
#[tokio::test]
async fn test_telegram_adapter_parse() {
    use hermes_platform_telegram::TelegramAdapter;

    let adapter = TelegramAdapter::new("bot".to_string(), "verify".to_string());

    let body = r#"{
        "update_id": 123456789,
        "message": {
            "message_id": 1,
            "from": {"id": 123456789, "is_bot": false, "first_name": "Test"},
            "chat": {"id": 123456789, "type": "private"},
            "text": "Hello"
        }
    }"#;

    let request = Request::builder()
        .uri("/webhook/telegram")
        .body(Body::from(body))
        .unwrap();

    let result = adapter.parse_inbound(request).await;
    assert!(result.is_ok(), "Should parse Telegram update: {:?}", result.err());

    let msg = result.unwrap();
    assert_eq!(msg.platform, "telegram");
    assert_eq!(msg.content, "Hello");
}

/// 测试 SMS 适配器解析
#[tokio::test]
async fn test_sms_adapter_parse() {
    use hermes_platform_sms::SmsAdapter;

    let adapter = SmsAdapter::new();

    let body = "From=%2B1234567890&To=%2B0987654321&Body=Test%20SMS";

    let request = Request::builder()
        .uri("/webhook/sms")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .unwrap();

    let result = adapter.parse_inbound(request).await;
    assert!(result.is_ok(), "Should parse SMS: {:?}", result.err());

    let msg = result.unwrap();
    assert_eq!(msg.platform, "sms");
    assert_eq!(msg.content, "Test SMS");
}

/// 测试 Feishu 适配器解析
#[tokio::test]
async fn test_feishu_adapter_parse() {
    use hermes_platform_feishu::FeishuAdapter;

    let adapter = FeishuAdapter::new();

    let body = serde_json::json!({
        "schema": "2.0",
        "header": {
            "event_id": "evt_123",
            "event_type": "im.message.receive_v1",
            "create_time": "1234567890000",
            "token": "test_token",
            "app_id": "cli_xxx",
            "tenant_key": "test_tenant"
        },
        "event": {
            "sender": {
                "sender_id": {
                    "user_id": "user_123"
                },
                "sender_type": "user"
            },
            "message": {
                "message_id": "msg_123",
                "chat_id": "chat_123",
                "msg_type": "text",
                "content": "{\"text\":\"Hello from Feishu\"}"
            }
        }
    }).to_string();

    let request = Request::builder()
        .uri("/webhook/feishu")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();

    let result = adapter.parse_inbound(request).await;
    assert!(result.is_ok(), "Should parse Feishu event: {:?}", result.err());

    let msg = result.unwrap();
    assert_eq!(msg.platform, "feishu");
}

/// 测试 DingTalk 适配器基本功能
#[tokio::test]
async fn test_dingtalk_adapter_parse() {
    use hermes_platform_dingtalk::DingTalkAdapter;

    let adapter = DingTalkAdapter::new();

    let body = serde_json::json!({
        "msgId": "msg_123",
        "senderNick": "Test User",
        "conversationId": "conv_123",
        "senderCid": "sender_123",
        "content": "Hello from DingTalk"
    }).to_string();

    let request = Request::builder()
        .uri("/webhook/dingtalk")
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap();

    let result = adapter.parse_inbound(request).await;
    assert!(result.is_ok(), "Should parse DingTalk message: {:?}", result.err());

    let msg = result.unwrap();
    assert_eq!(msg.platform, "dingtalk");
}

/// 测试 WeCom 适配器解析
/// 注意：WeCom 使用 AES-256-CBC 加密，需要完整的加密环境才能测试
/// 此测试需要与真实 WeCom 服务器交互来验证加密格式
#[ignore]
#[tokio::test]
async fn test_wecom_adapter_parse() {
    // WeCom 使用 XML + AES-256-CBC 加密，完整测试需要：
    // 1. 正确的 AES key（43位 Base64 编码）
    // 2. 正确的消息格式：random(16) + msg_len(4) + content + PKCS7 padding
    // 3. 正确的 CBC 加密流程
    // 实际使用中，WeCom webhook 会包含服务器正确加密的消息
    todo!("WeCom requires AES-256-CBC encryption - needs full crypto setup with WeCom server")
}
