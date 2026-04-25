# Weixin 适配器

微信公众平台/企业微信机器人适配器。

## 功能特性

- **Webhook 模式**: 支持 HTTP Webhook 回调
- **XML 解析**: 微信使用 XML 格式通信
- **消息类型**: 文本、图片、语音、视频、位置、链接
- **AES 加密**: 支持 AES-128-CBC 消息加密
- **Access Token**: 自动管理和刷新 Access Token

## 配置

### 配置文件

```toml
[[gateway.platforms.weixin]]
wx_app_id = "wx1234567890abcdef"
wx_app_secret = "your-app-secret"
wx_token = "your-token"
wx_aes_key = "your-aes-key"
```

### 环境变量

```bash
export HERMES_WEIXIN_APP_ID="wx1234567890abcdef"
export HERMES_WEIXIN_APP_SECRET="your-app-secret"
export HERMES_WEIXIN_TOKEN="your-token"
export HERMES_WEIXIN_AES_KEY="your-aes-key"
```

## 消息类型

| 类型 | 说明 |
|------|------|
| `text` | 文本消息 |
| `image` | 图片消息 |
| `voice` | 语音消息 |
| `video` | 视频消息 |
| `shortvideo` | 短视频消息 |
| `location` | 位置消息 |
| `link` | 链接消息 |
| `event` | 事件推送 |

## Webhook 路由

```
POST /webhook/weixin
GET  /webhook/weixin?echostr=xxx  # 微信服务器验证
```

## 消息格式

### 入站消息 (InboundMessage)

微信使用 XML 格式：

```xml
<xml>
  <ToUserName><![CDATA[toUser]]></ToUserName>
  <FromUserName><![CDATA[fromUser]]></FromUserName>
  <CreateTime>12345678</CreateTime>
  <MsgType><![CDATA[text]]></MsgType>
  <Content><![CDATA[content]]></Content>
  <MsgId>1234567890</MsgId>
</xml>
```

解析后：

```json
{
  "platform": "weixin",
  "sender_id": "open_id",
  "content": "消息内容",
  "session_id": "weixin:open_id",
  "timestamp": "2024-01-01T00:00:00Z",
  "raw": { /* XML 解析后的对象 */ }
}
```

### 出站消息

```rust
// 发送文本
weixin_client.send_text("open_id", "你好")?;

// 发送图片
weixin_client.send_image("open_id", "media_id")?;

// 发送模板消息
weixin_client.send_template("open_id", template_id, data)?;
```

## 签名验证

微信公众平台验证 URL 时使用 SHA-1 签名：

1. 将 token、timestamp、nonce 按字典序排列
2. 拼接后计算 SHA-1 签名
3. 与 `signature` 参数比对

企业微信使用 AES-128-CBC 加密：

```rust
// 解密步骤
1. 将 encoded_aes_key 进行 Base64 解码
2. 使用 AES-128-CBC 解密
3. 验证 appid
4. 提取随机字符串和消息内容
```

## Access Token 管理

Access Token 是调用微信 API 的凭证，有效期 2 小时。

```rust
// 自动获取和缓存
let token = weixin_client.get_access_token().await?;
```

## 依赖

```toml
[dependencies]
hermes-platform-weixin = { path = "crates/hermes-platform-weixin" }
```
