# SMS (Twilio) 适配器

Twilio SMS Webhook 适配器，支持接收和发送短信。

## 功能特性

- **Webhook 接收**: 接收入站 SMS
- **REST API 发送**: 通过 Twilio API 发送短信
- **签名验证**: HMAC-SHA1 签名验证
- **长消息分片**: 自动分割超过 1600 字符的消息
- **Basic 认证**: 使用 Twilio Account SID 和 Auth Token

## 配置

### 配置文件

```toml
[[gateway.platforms.sms]]
twilio_account_sid = "ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
twilio_auth_token = "your-auth-token"
twilio_from_number = "+1234567890"
```

### 环境变量

```bash
export HERMES_TWILIO_ACCOUNT_SID="ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
export HERMES_TWILIO_AUTH_TOKEN="your-auth-token"
export HERMES_TWILIO_FROM_NUMBER="+1234567890"
```

## Webhook 路由

```
POST /webhook/sms
```

## 消息格式

### 入站消息 (InboundMessage)

Twilio 使用 Form-encoded 格式：

```
From=%2B1234567890&To=%2B0987654321&Body=Hello
```

解析后：

```json
{
  "platform": "sms",
  "sender_id": "+1234567890",
  "content": "Hello",
  "session_id": "sms:+0987654321",
  "timestamp": "2024-01-01T00:00:00Z",
  "raw": {
    "from": "+1234567890",
    "to": "+0987654321",
    "body": "Hello",
    "message_sid": "SMxxxxx"
  }
}
```

### 出站消息

```rust
// 发送短信
twilio_client.send_message("+1234567890", "Hello, World!")?;

// 自动处理长消息分片
twilio_client.send_message("+1234567890", long_message)?;
// 如果超过 1600 字符，会自动分割并标注 (1/2), (2/2)
```

## Webhook 签名验证

Twilio 使用 HMAC-SHA1 签名验证请求：

```rust
// 验证函数
pub fn verify_signature(
    url: &str,
    params: &[(String, String)],
    signature: &str,
) -> bool {
    // 1. 按键名排序参数
    // 2. 拼接 URL + 参数
    // 3. 使用 HMAC-SHA1 计算签名
    // 4. Base64 编码后比对
}
```

验证头：`X-Twilio-Signature`

## 消息分片

Twilio 单条短信最多 1600 字符（GSM-7）或 670 字符（UCS-2）。

适配器自动处理长消息：

```rust
fn split_message(message: &str, max_len: usize) -> Vec<String> {
    // 消息被分割成多个片段
    // 每个片段末尾添加分片编号：(1/2), (2/2)
}
```

## Twilio Webhook Payload 字段

| 字段 | 说明 |
|------|------|
| `From` | 发件人号码 |
| `To` | 收件人号码 |
| `Body` | 消息内容 |
| `MessageSid` | 消息 SID |
| `AccountSid` | 账户 SID |
| `FromCity` | 发件人城市 |
| `FromState` | 发件人州/省 |
| `FromCountry` | 发件人国家 |
| `ToCity` | 收件人城市 |
| `ToState` | 收件人州/省 |
| `ToCountry` | 收件人国家 |

## 依赖

```toml
[dependencies]
hermes-platform-sms = { path = "crates/hermes-platform-sms" }
```

## 错误处理

```rust
pub enum SmsError {
    Auth(String),           // 认证失败
    Api(String),            // API 错误
    NotAuthenticated,       // 未配置凭据
    Parse(String),          // 解析错误
    InvalidSignature,       // 签名验证失败
    Network(String),        // 网络错误
    SendMessage(String),    // 发送失败
}
```
