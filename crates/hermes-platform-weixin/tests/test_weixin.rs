use hermes_core::gateway::PlatformAdapter;
use hermes_platform_weixin::WeixinAdapter;

#[tokio::test]
async fn test_adapter_name() {
    let adapter = WeixinAdapter::new();
    assert_eq!(adapter.platform_id(), "weixin");
}

#[tokio::test]
async fn test_adapter_with_credentials() {
    let adapter = WeixinAdapter::new()
        .with_credentials("test_app_id".to_string(), "test_secret".to_string());
    assert_eq!(adapter.platform_id(), "weixin");
}
