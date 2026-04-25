# Telegram 适配器

Telegram Bot API 适配器。

## 功能特性

- **Webhook 模式**: 支持 HTTP Webhook 回调
- **REST API**: 支持主动发送消息
- **消息类型**: 文本、图片、音频、视频、文档、位置、联系人
- **Keyboard**: 支持 Inline Keyboard 和 Reply Keyboard

## 配置

### 配置文件

```toml
[[gateway.platforms.telegram]]
bot_token = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
verify_token = "your-verify-token"  # 可选，用于 webhook 验证
```

### 环境变量

```bash
export HERMES_TELEGRAM_BOT_TOKEN="123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11"
export HERMES_TELEGRAM_VERIFY_TOKEN="your-verify-token"
```

## 消息类型

| 类型 | 说明 |
|------|------|
| `text` | 文本消息 |
| `photo` | 图片 |
| `audio` | 音频 |
| `voice` | 语音 |
| `video` | 视频 |
| `document` | 文档 |
| `location` | 位置 |
| `contact` | 联系人 |
| `sticker` | 表情 |

## Webhook 路由

```
POST /webhook/telegram
GET  /webhook/telegram?secret_token=xxx  # Telegram 验证
```

## 消息格式

### 入站消息 (InboundMessage)

```json
{
  "platform": "telegram",
  "sender_id": "123456789",
  "content": "消息内容",
  "session_id": "telegram:123456789",
  "timestamp": "2024-01-01T00:00:00Z",
  "raw": { /* 原始 Telegram Update */ }
}
```

### 出站消息

```rust
// 发送文本
telegram_client.send_message(chat_id, "Hello")?;

// 发送图片
telegram_client.send_photo(chat_id, "photo_file_id")?;

// 发送 Markdown
telegram_client.send_message(chat_id, "*bold* _italic_")?;

// 发送回复键盘
telegram_client.send_reply_keyboard(chat_id, "Choose:", buttons)?;
```

## Inline Keyboard

```rust
use hermes_platform_telegram::types::InlineKeyboardButton;

let keyboard = vec![
    vec![
        InlineKeyboardButton::new("选项1", "callback_data_1"),
        InlineKeyboardButton::new("选项2", "callback_data_2"),
    ]
];

telegram_client.send_inline_keyboard(chat_id, "选择一个:", keyboard)?;
```

## Webhook 验证

Telegram 使用 `secret_token` 验证请求：

```
GET /webhook/telegram?secret_token=your_secret_token
```

验证流程：
1. 检查 `secret_token` 与配置是否匹配
2. Telegram 会在 1 秒内响应空 Body 表示验证成功

## 依赖

```toml
[dependencies]
hermes-platform-telegram = { path = "crates/hermes-platform-telegram" }
```
