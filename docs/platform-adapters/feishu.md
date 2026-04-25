# Feishu 适配器

飞书（Lark）自建应用机器人适配器。

## 功能特性

- **Webhook 模式**: 支持 HTTP Webhook 回调
- **REST API**: 支持主动发送消息
- **消息类型**: 文本、Post、图片、音频、视频、文件、表情
- **签名验证**: AES-256-CBC 加密 + HMAC-SHA256 签名

## 配置

### 配置文件

```toml
[[gateway.platforms.feishu]]
feishu_app_id = "cli_xxxxxxxxx"
feishu_app_secret = "your-app-secret"
verification_token = "your-verification-token"
encrypt_key = "your-encrypt-key"
```

### 环境变量

```bash
export HERMES_FEISHU_APP_ID="cli_xxxxxxxxx"
export HERMES_FEISHU_APP_SECRET="your-app-secret"
export HERMES_FEISHU_VERIFICATION_TOKEN="your-verification-token"
export HERMES_FEISHU_ENCRYPT_KEY="your-encrypt-key"
```

## 消息类型

| 类型 | 说明 |
|------|------|
| `text` | 纯文本 |
| `post` | 富文本 |
| `image` | 图片 |
| `audio` | 音频 |
| `video` | 视频 |
| `file` | 文件 |
| `sticker` | 表情 |

## Webhook 路由

```
POST /webhook/feishu
```

## 消息格式

### 入站消息 (InboundMessage)

```json
{
  "platform": "feishu",
  "sender_id": "user_id or open_id",
  "content": "消息内容",
  "session_id": "feishu:chat_id",
  "timestamp": "2024-01-01T00:00:00Z",
  "raw": { /* 原始飞书事件 */ }
}
```

### 出站消息

```rust
// 发送文本
feishu_client.send_text("chat_id", "你好")?;

// 发送富文本
feishu_client.send_post("chat_id", post_content)?;

// 发送图片
feishu_client.send_image("chat_id", "image_key")?;
```

## 签名验证

飞书使用两种安全机制：

1. **Verification Token**: 验证事件来自飞书
2. **Encrypt Key**: AES-256-CBC 加密消息体

验证流程：
1. 解密 `encrypt_key` 解密请求体
2. 验证 `timestamp` 未过期
3. 验证签名 `HMAC-SHA256(encrypt_key, timestamp + "." + plaintext)`

## 依赖

```toml
[dependencies]
hermes-platform-feishu = { path = "crates/hermes-platform-feishu" }
```
