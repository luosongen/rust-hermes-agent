//! 钉钉平台适配器测试

use hermes_core::gateway::PlatformAdapter;
use hermes_platform_dingtalk::DingTalkAdapter;

#[tokio::test]
async fn test_adapter_id() {
    let adapter = DingTalkAdapter::new();
    assert_eq!(adapter.platform_id(), "dingtalk");
}

#[tokio::test]
async fn test_adapter_with_credentials() {
    let adapter = DingTalkAdapter::new()
        .with_credentials("test_client_id".to_string(), "test_secret".to_string());
    assert_eq!(adapter.platform_id(), "dingtalk");
}
