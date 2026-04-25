# Platform Adapters 平台适配器

本文档介绍 hermes-agent 支持的消息平台适配器。

## 支持的平台

| 平台 | Webhook 路由 | 文档 |
|------|-------------|------|
| Telegram | `/webhook/telegram` | [telegram.md](./telegram.md) |
| WeCom | `/webhook/wecom` | [wecom.md](./wecom.md) |
| DingTalk | `/webhook/dingtalk` | [dingtalk.md](./dingtalk.md) |
| Feishu | `/webhook/feishu` | [feishu.md](./feishu.md) |
| Weixin | `/webhook/weixin` | [weixin.md](./weixin.md) |
| SMS (Twilio) | `/webhook/sms` | [sms.md](./sms.md) |

## 通用配置

所有平台适配器通过 `PlatformConfig` 配置，支持两种配置方式：

### 1. 配置文件 (`~/.config/hermes-agent/config.toml`)

```toml
[gateway]
port = 8080
host = "0.0.0.0"

[[gateway.platforms.<platform_name>]]
# 平台特定配置
```

### 2. 环境变量

```bash
export HERMES_<PLATFORM>_<CONFIG_NAME>="value"
```

## PlatformAdapter Trait

所有平台适配器实现 `PlatformAdapter` trait：

```rust
pub trait PlatformAdapter: Send + Sync {
    fn platform_id(&self) -> &'static str;
    fn verify_webhook(&self, request: &Request<Body>) -> bool;
    async fn parse_inbound(&self, request: Request<Body>) -> Result<InboundMessage, GatewayError>;
    async fn send_response(&self, response: ConversationResponse, message: &InboundMessage) -> Result<(), GatewayError>;
}
```

## 消息流程

```
Webhook 请求 → verify_webhook() → parse_inbound() → Agent.run_conversation()
                                                            ↓
                                              send_response() ← Agent 响应
```
