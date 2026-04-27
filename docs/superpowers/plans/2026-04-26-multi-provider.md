# Multi-Provider 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现多 provider 支持和 streaming — 根据 model ID 选择 provider，各 provider 独立实现 streaming

**Architecture:**
- CLI 层根据 model ID 前缀创建对应 provider，Agent 保持单一 provider 接口
- 每个 provider 独立实现 `chat_streaming()` 方法
- 配置通过 config.toml 管理多 provider 凭据

**Tech Stack:** Rust, reqwest, futures-util, serde, tokio

---

## 文件结构

```
hermes-cli/src/
├── chat.rs                    # 修改：根据 model ID 创建 provider

hermes-provider/src/
├── openai.rs                 # 修改：实现 chat_streaming
├── anthropic.rs              # 修改：实现 chat_streaming
├── openrouter.rs             # 修改：实现 chat_streaming
├── glm.rs                    # 修改：实现 chat_streaming
├── minimax.rs                # 修改：实现 chat_streaming
├── kimi.rs                   # 修改：实现 chat_streaming
├── deepseek.rs               # 修改：实现 chat_streaming
└── qwen.rs                  # 修改：实现 chat_streaming
```

---

## Task 1: 实现 Provider 工厂函数

**Files:**
- Modify: `hermes-cli/src/chat.rs`

- [ ] **Step 1: 添加 Provider 工厂函数**

在 `chat.rs` 顶部添加以下函数：

```rust
/// 根据 model ID 创建对应的 provider
fn create_provider_for_model(model: &str, api_key: Option<&str>) -> Result<Arc<dyn hermes_core::LlmProvider>, anyhow::Error> {
    let (provider_name, _) = model.split_once('/').unwrap_or((model, ""));

    match provider_name {
        "openai" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .or_else(|| std::env::var("HERMES_OPENAI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("OpenAI API key not found"))?;
            Ok(Arc::new(hermes_provider::OpenAiProvider::new(key, None)))
        }
        "anthropic" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Anthropic API key not found"))?;
            Ok(Arc::new(hermes_provider::AnthropicProvider::new(key, None)))
        }
        "openrouter" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("OpenRouter API key not found"))?;
            Ok(Arc::new(hermes_provider::OpenRouterProvider::new(key, None)))
        }
        "glm" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("GLM_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("GLM API key not found"))?;
            Ok(Arc::new(hermes_provider::GlmProvider::new(key, None)))
        }
        "minimax" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("MINIMAX_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("MiniMax API key not found"))?;
            Ok(Arc::new(hermes_provider::MinimaxProvider::new(key, None)))
        }
        "kimi" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("KIMI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Kimi API key not found"))?;
            Ok(Arc::new(hermes_provider::KimiProvider::new(key, None)))
        }
        "deepseek" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("DeepSeek API key not found"))?;
            Ok(Arc::new(hermes_provider::DeepSeekProvider::new(key, None)))
        }
        "qwen" => {
            let key = api_key
                .map(String::from)
                .or_else(|| std::env::var("QWEN_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Qwen API key not found"))?;
            Ok(Arc::new(hermes_provider::QwenProvider::new(key, None)))
        }
        _ => Err(anyhow::anyhow!("Unsupported provider: {}", provider_name)),
    }
}
```

- [ ] **Step 2: 修改 run_chat 函数使用工厂函数**

找到 `run_chat` 函数中的 provider 创建代码，替换为：

```rust
// 构建 LLM Provider
let provider: Arc<dyn LlmProvider> = if let Some(creds) = credentials {
    // 使用凭据字符串创建凭据池
    let pool = hermes_core::CredentialPool::new(PoolStrategy::RoundRobin);
    for cred in creds.split(',') {
        let parts: Vec<&str> = cred.split(':').collect();
        if parts.len() == 2 {
            pool.add(parts[0], parts[1], parts[1]);
        }
    }
    // 使用 RetryingProvider 包装
    let model_key = model.clone();
    let inner_provider = create_provider_for_model(&model_key, None)?;
    Arc::new(hermes_core::RetryingProvider::new(
        inner_provider,
        Arc::new(pool),
        hermes_core::RetryPolicy::default(),
    ))
} else {
    // 根据 model ID 创建对应的 provider
    create_provider_for_model(&model, credentials.as_deref())?
};
```

- [ ] **Step 3: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-cli`
Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-cli/src/chat.rs
git commit -m "feat(multi-provider): 根据 model ID 创建对应 provider"
```

---

## Task 2: OpenAI Streaming 实现

**Files:**
- Modify: `crates/hermes-provider/src/openai.rs`

- [ ] **Step 1: 添加 streaming 相关结构体**

在 `OpenAiRequest` 结构体后添加：

```rust
/// OpenAI Streaming Delta
#[derive(Debug, Deserialize)]
struct OpenAiStreamDelta {
    choices: Vec<OpenAiStreamChoice>,
}

/// OpenAI Streaming Choice
#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiStreamContent,
    finish_reason: Option<String>,
}

/// Streaming 内容
#[derive(Debug, Deserialize)]
struct OpenAiStreamContent {
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<OpenAiToolCall>>,
}
```

- [ ] **Step 2: 实现 chat_streaming 方法**

将现有的 `chat_streaming` 方法替换为：

```rust
async fn chat_streaming(
    &self,
    request: ChatRequest,
    callback: hermes_core::StreamingCallback,
) -> Result<ChatResponse, ProviderError> {
    let oai_request = self.convert_request(request);

    let mut full_content = String::new();
    let mut full_tool_calls: Vec<hermes_core::ToolCall> = Vec::new();
    let mut finish_reason = hermes_core::FinishReason::Stop;

    let response = self
        .client
        .post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", self.api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&oai_request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 {
            return Err(ProviderError::Auth);
        }
        if status.as_u16() == 429 {
            return Err(ProviderError::RateLimit(60));
        }
        return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
    }

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }
                if let Ok(delta) = serde_json::from_str::<OpenAiStreamDelta>(data) {
                    if let Some(choice) = delta.choices.into_iter().next() {
                        if let Some(content) = choice.delta.content {
                            full_content += &content;
                            callback(ChatResponse {
                                content: content.clone(),
                                finish_reason: hermes_core::FinishReason::Stop,
                                tool_calls: None,
                                reasoning: None,
                                usage: None,
                            });
                        }
                        if let Some(tool_calls) = choice.delta.tool_calls {
                            for tc in tool_calls {
                                if let Some(func) = tc.function {
                                    full_tool_calls.push(hermes_core::ToolCall {
                                        id: tc.index.to_string(),
                                        name: func.name,
                                        arguments: serde_json::from_str(&func.arguments.unwrap_or_default())
                                            .unwrap_or_default(),
                                    });
                                }
                            }
                        }
                        if let Some(ref fr) = choice.finish_reason {
                            finish_reason = match fr.as_str() {
                                "stop" => hermes_core::FinishReason::Stop,
                                "length" => hermes_core::FinishReason::Length,
                                "content_filter" => hermes_core::FinishReason::ContentFilter,
                                _ => hermes_core::FinishReason::Other,
                            };
                        }
                    }
                }
            }
        }
    }

    Ok(ChatResponse {
        content: full_content,
        finish_reason,
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

- [ ] **Step 3: 确保 futures_util 在作用域**

在文件顶部添加：

```rust
use futures_util::StreamExt;
```

如果已经存在，则确保在 `chat_streaming` 方法内使用。

- [ ] **Step 4: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-provider`
Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-provider/src/openai.rs
git commit -m "feat(provider): OpenAI streaming 实现"
```

---

## Task 3: Anthropic Streaming 实现

**Files:**
- Modify: `crates/hermes-provider/src/anthropic.rs`

- [ ] **Step 1: 查看现有 chat_streaming 实现**

```rust
async fn chat_streaming(
    &self,
    _request: ChatRequest,
    _callback: hermes_core::StreamingCallback,
) -> Result<ChatResponse, ProviderError> {
    Err(ProviderError::Api("Streaming not yet implemented".into()))
}
```

- [ ] **Step 2: 添加 Anthropic streaming 结构体**

在文件顶部添加：

```rust
use futures_util::StreamExt;

#[derive(Debug, Deserialize)]
struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    index: Option<usize>,
    delta: Option<AnthropicStreamDelta>,
    #[serde(rename = "stop_reason")]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicStreamDelta {
    #[serde(rename = "type")]
    delta_type: String,
    text: Option<String>,
    #[serde(rename = "partial_json")]
    partial_json: Option<String>,
}
```

- [ ] **Step 3: 实现 chat_streaming**

替换现有的 `chat_streaming` 方法：

```rust
async fn chat_streaming(
    &self,
    request: ChatRequest,
    callback: hermes_core::StreamingCallback,
) -> Result<ChatResponse, ProviderError> {
    let anthropic_request = self.convert_request(request);

    let mut full_content = String::new();
    let mut full_tool_calls: Vec<hermes_core::ToolCall> = Vec::new();
    let mut stop_reason = "end_turn".to_string();

    let response = self
        .client
        .post(format!("{}/v1/messages", self.base_url))
        .header("x-api-key", &self.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .json(&anthropic_request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if status.as_u16() == 401 {
            return Err(ProviderError::Auth);
        }
        if status.as_u16() == 429 {
            return Err(ProviderError::RateLimit(60));
        }
        return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
    }

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if let Ok(event) = serde_json::from_str::<AnthropicStreamEvent>(data) {
                    match event.event_type.as_str() {
                        "content_block_delta" => {
                            if let Some(delta) = event.delta {
                                if delta.delta_type == "text_delta" {
                                    if let Some(text) = delta.text {
                                        full_content += &text;
                                        callback(ChatResponse {
                                            content: text.clone(),
                                            finish_reason: hermes_core::FinishReason::Stop,
                                            tool_calls: None,
                                            reasoning: None,
                                            usage: None,
                                        });
                                    }
                                } else if delta.delta_type == "input_json_delta" {
                                    if let Some(json) = delta.partial_json {
                                        let idx = event.index.unwrap_or(0);
                                        if full_tool_calls.len() <= idx {
                                            full_tool_calls.resize(idx + 1, hermes_core::ToolCall {
                                                id: format!("tool-{}", idx),
                                                name: String::new(),
                                                arguments: std::collections::HashMap::new(),
                                            });
                                        }
                                        let args = &mut full_tool_calls[idx].arguments;
                                        let current: String = args.iter()
                                            .map(|(k,v)| format!("{}: {}", k, v))
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        args.insert(format!("__partial_{}", idx), serde_json::Value::String(current + &json));
                                    }
                                }
                            }
                        }
                        "message_stop" => {
                            if let Some(ref sr) = event.stop_reason {
                                stop_reason = sr.clone();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let finish_reason = match stop_reason.as_str() {
        "end_turn" => hermes_core::FinishReason::Stop,
        "max_tokens" => hermes_core::FinishReason::Length,
        _ => hermes_core::FinishReason::Other,
    };

    Ok(ChatResponse {
        content: full_content,
        finish_reason,
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

- [ ] **Step 4: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-provider`
Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add crates/hermes-provider/src/anthropic.rs
git commit -m "feat(provider): Anthropic streaming 实现"
```

---

## Task 4-9: 其他 Provider Streaming 实现

对于 OpenRouter、GLM、MiniMax、Kimi、DeepSeek、Qwen，这些 provider 都是 OpenAI-compatible 的，使用类似的 SSE 格式。

**通用模式（在每个文件中）：**

- [ ] **Step 1: 添加 `use futures_util::StreamExt;`**

- [ ] **Step 2: 替换 `chat_streaming` 方法**

由于它们是 OpenAI-compatible 的，复用 OpenAI 的 streaming 逻辑：

```rust
async fn chat_streaming(
    &self,
    request: ChatRequest,
    callback: hermes_core::StreamingCallback,
) -> Result<ChatResponse, ProviderError> {
    // 使用 OpenAI-compatible 格式
    let oai_request = convert_to_openai_request(request);

    let mut full_content = String::new();
    let mut full_tool_calls: Vec<hermes_core::ToolCall> = Vec::new();

    let response = self
        .client
        .post(format!("{}/chat/completions", self.base_url))
        .header("Authorization", format!("Bearer {}", self.api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&oai_request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
    }

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }
                // 解析 delta...
                // 调用 callback...
            }
        }
    }

    Ok(ChatResponse {
        content: full_content,
        finish_reason: hermes_core::FinishReason::Stop,
        tool_calls: if full_tool_calls.is_empty() { None } else { Some(full_tool_calls) },
        reasoning: None,
        usage: None,
    })
}
```

**需要修改的文件和提交：**

| Task | Provider | Commit |
|------|----------|--------|
| Task 4 | OpenRouter | `feat(provider): OpenRouter streaming 实现` |
| Task 5 | GLM | `feat(provider): GLM streaming 实现` |
| Task 6 | MiniMax | `feat(provider): MiniMax streaming 实现` |
| Task 7 | Kimi | `feat(provider): Kimi streaming 实现` |
| Task 8 | DeepSeek | `feat(provider): DeepSeek streaming 实现` |
| Task 9 | Qwen | `feat(provider): Qwen streaming 实现` |

---

## Task 10: 运行完整测试

- [ ] **Step 1: 运行 hermes-provider 测试**

Run: `cargo test -p hermes-provider 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 2: 运行 hermes-cli 测试**

Run: `cargo test -p hermes-cli 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 3: 运行 cargo check 验证所有 crates**

Run: `cargo check --all 2>&1 | grep "^error" | head -10`
Expected: 无错误

- [ ] **Step 4: 提交最终变更**

```bash
git add -A
git commit -m "feat(multi-provider): 完成多 provider 支持和 streaming 实现

- Provider 工厂根据 model ID 创建对应 provider
- OpenAI/Anthropic 实现 streaming
- OpenRouter/GLM/MiniMax/Kimi/DeepSeek/Qwen 实现 streaming
Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## 成功标准检查清单

- [ ] CLI 根据 model ID（如 `anthropic/claude-3-5-sonnet`）创建正确的 provider
- [ ] OpenAI provider 的 `chat_streaming()` 返回流式响应
- [ ] Anthropic provider 的 `chat_streaming()` 返回流式响应
- [ ] 其他 provider streaming 正常工作
- [ ] 向后兼容：单 provider 模式仍然正常工作
