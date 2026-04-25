# WeCom 适配器

企业微信（WeCom）应用适配器。

## 功能特性

- **Webhook 模式**: 支持 HTTP Webhook 回调
- **REST API**: 支持主动发送消息
- **消息类型**: 文本、图片、音频、视频、文件、Markdown
- **AES 加密**: AES-256-CBC 消息加密
- **企业会话**: 支持群聊和单聊

## 配置

### 配置文件

```toml
[[gateway.platforms.wecom]]
corp_id = "ww1234567890abcdef"
agent_id = "1000001"
token = "your-token"
aes_key = "your-aes-key"
```

### 环境变量

```bash
export HERMES_WECOM_CORP_ID="ww1234567890abcdef"
export HERMES_WECOM_AGENT_ID="1000001"
export HERMES_WECOM_TOKEN="your-token"
export HERMES_WECOM_AES_KEY="your-aes-key"
```

## 消息类型

| 类型 | 说明 |
|------|------|
| `text` | 文本消息 |
| `image` | 图片消息 |
| `voice` | 语音消息 |
| `video` | 视频消息 |
| `file` | 文件消息 |
| `textcard` | 卡片消息 |
| `markdown` | Markdown 消息 |
| `news` | 图文消息 |

## Webhook 路由

```
POST /webhook/wecom
GET  /webhook/wecom?msg_signature=xxx&timestamp=xxx&nonce=xxx&echostr=xxx  # 验证
```

## 消息格式

### 入站消息 (InboundMessage)

企业微信使用 XML 格式：

```xml
<xml>
  <ToUserName><![CDATA[CorpID]]></ToUserName>
  <FromUserName><![CDATA[UserID]]></FromUserName>
  <CreateTime>12345678</CreateTime>
  <MsgType><![CDATA[text]]></MsgType>
  <Content><![CDATA[content]]></Content>
  <MsgId>1234567890</MsgId>
  <AgentID>1000001</AgentID>
</xml>
```

解析后：

```json
{
  "platform": "wecom",
  "sender_id": "UserID",
  "content": "消息内容",
  "session_id": "wecom:UserID",
  "timestamp": "2024-01-01T00:00:00Z",
  "raw": { /* XML 解析后的对象 */ }
}
```

### 出站消息

```rust
// 发送文本
wecom_client.send_text("user_id", "你好")?;

// 发送图片
wecom_client.send_image("user_id", "media_id")?;

// 发送 Markdown
wecom_client.send_markdown("user_id", "**粗体** _斜体_")?;

// 发送卡片消息
wecom_client.send_textcard("user_id", textcard_content)?;
```

## AES 加密解密

企业微信使用 AES-256-CBC 加密消息：

```rust
// 解密步骤
1. 从请求获取 msg_signature, timestamp, nonce, echostr
2. 拼接字符串：timestamp + "\n" + nonce + "\n" + echostr
3. 计算 SHA-1 签名并与 msg_signature 比对
4. Base64 解码 echostr
5. 使用 AES-256-CBC 解密，PKCS7 填充
6. 提取随机字符串 (16字节) + 消息长度 (4字节) + 消息内容

// 加密步骤
1. 随机生成 16 字节字符串
2. 拼接：random(16) + msg_len(4) + from_id + msg + appid
3. 补足 32 字节倍数
4. AES-256-CBC 加密
5. Base64 编码
```

## Access Token

企业微信 API 调用需要 Access Token：

```rust
// 获取 Access Token
let token = wecom_client.get_access_token().await?;

// Access Token 有效期 2 小时，自动刷新
```

## 依赖

```toml
[dependencies]
hermes-platform-wecom = { path = "crates/hermes-platform-wecom" }
```
