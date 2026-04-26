# Multi-Provider 支持设计方案

> **Goal:** 让 Hermes Agent 支持多个 LLM provider，根据 model ID 自动选择对应 provider，并实现 streaming 输出

> **Architecture:** CLI 层根据 model ID 创建对应 provider，Agent 保持单一 provider 接口；各 provider 独立实现 streaming

> **Tech Stack:** Rust, reqwest, serde, tokio

---

## 1. 概述

### 1.1 当前状态

- `LlmProvider` trait 定义完整
- 8 个 provider 实现已存在：OpenAI, Anthropic, OpenRouter, GLM, MiniMax, Kimi, DeepSeek, Qwen
- Agent 持有单一 `Arc<dyn LlmProvider>`，不感知多 provider
- 所有 provider 的 `chat_streaming()` 返回错误，未实现
- CLI 只创建 OpenAI provider

### 1.2 目标

1. **多 provider 支持** — 根据 model ID 选择对应 provider
2. **Streaming 实现** — Provider 端实现 `chat_streaming()`
3. **配置驱动** — 通过 config.toml 配置多 provider 凭据

---

## 2. 架构设计

### 2.1 Provider 路由

```
CLI (chat.rs)
    │
    ├─► 解析 model ID（如 "anthropic/claude-3-5-sonnet-20241022"）
    │
    ├─► 根据 provider 前缀创建对应 provider
    │     openai/*       → OpenAiProvider
    │     anthropic/*    → AnthropicProvider
    │     openrouter/*   → OpenRouterProvider
    │     glm/*         → GlmProvider
    │     minimax/*     → MinimaxProvider
    │     kimi/*        → KimiProvider
    │     deepseek/*     → DeepSeekProvider
    │     qwen/*        → QwenProvider
    │
    └─► Agent(provider) ← 只看到单一 provider
```

### 2.2 Agent 不变原则

Agent 结构保持不变：
```rust
pub struct Agent {
    provider: Arc<dyn LlmProvider>,  // 单一 provider
    // ...
}
```

CLI 负责根据 model ID 创建正确的 provider 实例，Agent 无需感知多 provider 路由逻辑。

### 2.3 Config 扩展

`config.toml` 新增 provider 配置：

```toml
[providers]
default = "openai/gpt-4o"

[providers.openai]
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"

[providers.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
base_url = "https://api.anthropic.com"

[providers.openrouter]
api_key = "${OPENROUTER_API_KEY}"
base_url = "https://openrouter.ai/api/v1"

[providers.glm]
api_key = "${GLM_API_KEY}"
base_url = "https://open.bigmodel.cn/api/paas/v4"

[providers.minimax]
api_key = "${MINIMAX_API_KEY}"
base_url = "https://api.minimax.chat/v1"

[providers.kimi]
api_key = "${KIMI_API_KEY}"
base_url = "https://api.moonshot.cn/v1"

[providers.deepseek]
api_key = "${DEEPSEEK_API_KEY}"
base_url = "https://api.deepseek.com"

[providers.qwen]
api_key = "${QWEN_API_KEY}"
base_url = "https://dashscope.aliyuncs.com/api/v1"
```

环境变量引用格式 `${VAR_NAME}`，运行时替换。

---

## 3. Streaming 实现

### 3.1 各 provider 独立实现

每个 provider 文件中独立实现 `chat_streaming()`，使用 `reqwest` SSE。

### 3.2 OpenAI Streaming 模式

```rust
async fn chat_streaming(
    &self,
    request: ChatRequest,
    callback: StreamingCallback,
) -> Result<ChatResponse, ProviderError> {
    // 1. 构建请求
    let chat_request = convert_to_openai_request(request);

    // 2. 发送 SSE 请求
    let response = self.client
        .post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", self.api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&chat_request)
        .send()
        .await?;

    // 3. 解析 SSE 流
    let mut full_content = String::new();
    let mut full_tool_calls = Vec::new();

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
        let text = String::from_utf8_lossy(&bytes);

        // 解析 SSE 行: data: {...}
        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }
                if let Ok(delta) = serde_json::from_str::<OpenAiStreamDelta>(data) {
                    full_content += &delta.content.unwrap_or_default();

                    // 处理 tool_calls
                    if let Some(tc) = delta.tool_calls {
                        for call in tc {
                            full_tool_calls.push(call);
                        }
                    }

                    // 调用 callback
                    callback(ChatResponse {
                        content: delta.content.unwrap_or_default(),
                        finish_reason: FinishReason::Stop,
                        tool_calls: None,
                        reasoning: None,
                        usage: None,
                    });
                }
            }
        }
    }

    // 4. 返回完整响应
    Ok(ChatResponse {
        content: full_content,
        finish_reason: FinishReason::Stop,
        tool_calls: if full_tool_calls.is_empty() {
            None
        } else {
            Some(full_tool_calls)
        },
        reasoning: None,
        usage: None,
    })
}
```

### 3.3 Anthropic Streaming 模式

Anthropic 使用不同的 SSE 格式：

```rust
// Anthropic stream 格式
// data: {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}
// data: {"type": "message_stop"}
```

---

## 4. 文件变更

### 4.1 CLI 层

| 文件 | 变更 |
|------|------|
| `hermes-cli/src/chat.rs` | 根据 model ID 前缀创建对应 provider |
| `hermes-core/src/config/` | 扩展 ProviderSettings 支持多 provider 配置 |

### 4.2 Provider 层

| 文件 | 变更 |
|------|------|
| `hermes-provider/src/openai.rs` | 实现 chat_streaming |
| `hermes-provider/src/anthropic.rs` | 实现 chat_streaming |
| `hermes-provider/src/openrouter.rs` | 实现 chat_streaming |
| `hermes-provider/src/glm.rs` | 实现 chat_streaming |
| `hermes-provider/src/minimax.rs` | 实现 chat_streaming |
| `hermes-provider/src/kimi.rs` | 实现 chat_streaming |
| `hermes-provider/src/deepseek.rs` | 实现 chat_streaming |
| `hermes-provider/src/qwen.rs` | 实现 chat_streaming |

---

## 5. 依赖

无新依赖。使用现有的：
- `reqwest`（已有）
- `futures-util`（已有，用于 StreamExt）
- `serde`

---

## 6. 成功标准

1. CLI 根据 model ID（如 `anthropic/claude-3-5-sonnet`）创建正确的 provider
2. 配置文件中可以设置多个 provider 的 API key
3. OpenAI provider 的 `chat_streaming()` 返回流式响应
4. Anthropic provider 的 `chat_streaming()` 返回流式响应
5. 其他 provider（OpenRouter, GLM, MiniMax, Kimi, DeepSeek, Qwen）的 `chat_streaming()` 返回流式响应
6. 向后兼容：单 provider 模式仍然正常工作
