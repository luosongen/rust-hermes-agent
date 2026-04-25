# DingTalk 适配器

钉钉企业内部开发机器人适配器。

## 功能特性

- **Stream 模式**: 支持 WebSocket 长连接推送
- **Webhook 模式**: 支持 HTTP Webhook 回调
- **消息类型**: 文本、图片、音频、视频、表情
- **签名验证**: HMAC-SHA256 签名验证

## 配置

### 配置文件

```toml
[[gateway.platforms.dingtalk]]
app_key = "dinggmlmfxxxxx"
app_secret = "your-app-secret"
```

### 环境变量

```bash
export HERMES_DINGTALK_APP_KEY="dinggmlmfxxxxx"
export HERMES_DINGTALK_APP_SECRET="your-app-secret"
```

## 消息类型

| 类型 | 说明 |
|------|------|
| `text` | 文本消息 |
| `image` | 图片消息 |
| `voice` | 语音消息 |
| `video` | 视频消息 |
| `file` | 文件消息 |
| `link` | 链接消息 |

## Webhook 路由

```
POST /webhook/dingtalk
```

## 消息格式

### 入站消息 (InboundMessage)

```json
{
  "platform": "dingtalk",
  "sender_id": "sender_id",
  "content": "消息内容",
  "session_id": "dingtalk:conversation_id",
  "timestamp": "2024-01-01T00:00:00Z",
  "raw": { /* 原始钉钉消息 */ }
}
```

### 出站消息

通过 REST API 发送，支持多种消息类型：

```rust
// 发送文本消息
dingtalk_client.send_text("conversation_id", "你好");

// 发送图片
dingtalk_client.send_image("conversation_id", "image_id");

// 发送 Markdown
dingtalk_client.send_markdown("conversation_id", "# 标题\n内容");
```

## 签名验证

钉钉使用 HMAC-SHA256 签名验证请求合法性：

1. 获取请求头 `x-dingtalk-signature`
2. 获取请求头 `x-dingtalk-timestamp`
3. 验证签名：`HMAC-SHA256(timestamp + "\n" + appSecret, appSecret)`

## 依赖

```toml
[dependencies]
hermes-platform-dingtalk = { path = "crates/hermes-platform-dingtalk" }
```
