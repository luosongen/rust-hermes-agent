# Phase 1: LLM Providers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 6 个新的 LLM Provider，支持 OpenRouter、GLM、MiniMax、Qwen、Kimi、DeepSeek 模型

**Architecture:** 每个 Provider 独立实现为 `crates/hermes-provider/src/` 下的单个文件，遵循 `LlmProvider` trait。OpenRouter 是最通用的实现（OpenAI 兼容），其他 Provider 根据各自 API 特点实现。

**Tech Stack:** Rust, reqwest, async_trait, serde, hermes_core

---

## 实现顺序

1. **OpenRouter** - 通用 OpenAI 兼容 API，最简单
2. **GLM** - OpenAI 兼容，有微小差异
3. **MiniMax** - OpenAI 兼容
4. **Qwen** - OpenAI 兼容
5. **Kimi** - OpenAI 兼容
6. **DeepSeek** - OpenAI 兼容

> 注：GLM、MiniMax、Qwen、Kimi、DeepSeek 都声称 OpenAI 兼容，API 格式与 OpenAI 几乎相同，主要差异在认证、base_url 和模型列表。

---

## 共享代码提取

创建 `crates/hermes-provider/src/common.rs` 提取共享代码：

**Files:**
- Create: `crates/hermes-provider/src/common.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

```rust
//! 共享工具函数
//!
//! 提供 Provider 通用的请求/响应转换和错误处理

use hermes_core::{ChatRequest, Content, Message, ProviderError, Role};
use std::collections::HashMap;

/// 将 ChatRequest 中的消息转换为 OpenAI 格式
pub fn convert_messages(messages: &[Message]) -> Vec<OpenAiMessage> {
    messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            }
            .to_string();

            let content = match &m.content {
                Content::Text(t) => t.clone(),
                Content::Image { url, .. } => url.clone(),
                Content::ToolResult { content, .. } => content.clone(),
            };

            OpenAiMessage { role, content }
        })
        .collect()
}

/// 提取响应内容
pub fn extract_content(response: &OpenAiResponse) -> Result<String, ProviderError> {
    response
        .choices
        .iter()
        .next()
        .map(|c| c.message.content.clone())
        .ok_or_else(|| ProviderError::Api("No choices in response".into()))
}

/// 转换工具调用
pub fn convert_tool_calls(
    tool_calls: Option<Vec<OpenAiToolCall>>,
) -> Option<Vec<hermes_core::ToolCall>> {
    tool_calls.map(|tcs| {
        tcs.into_iter()
            .map(|tc| {
                let arguments: HashMap<String, serde_json::Value> =
                    serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                ToolCall {
                    id: tc.id,
                    name: tc.function.name,
                    arguments,
                }
            })
            .collect()
    })
}
```

---

## Task 1: OpenRouter Provider

**Files:**
- Create: `crates/hermes-provider/src/openrouter.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

- [ ] **Step 1: 创建 openrouter.rs 文件**

```rust
use async_trait::async_trait;
use crate::common;
use crate::common::{convert_messages, convert_tool_calls, extract_content};
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// OpenRouter Provider
///
/// API: https://openrouter.ai/docs
/// OpenRouter 是 OpenAI 兼容 API，支持 200+ 模型
///
/// 认证方式: API Key 在 Authorization header
/// Base URL: https://openrouter.ai/api/v1

pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
}

impl OpenRouterProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
        }
    }
}

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    stream: bool,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    id: String,
    choices: Vec<OpenRouterChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

// OpenAI 兼容类型
#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "function")]
    function: OpenAiFunctionCall,
}

#[derive(Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[async_trait]
impl hermes_core::LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("openrouter", "openai/gpt-4o"),
            ModelId::new("openrouter", "openai/gpt-4-turbo"),
            ModelId::new("openrouter", "anthropic/claude-3.5-sonnet"),
            ModelId::new("openrouter", "google/gemini-pro-1.5"),
            ModelId::new("openrouter", "meta-llama/llama-3-70b-instruct"),
        ]
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = "https://openrouter.ai/api/v1/chat/completions";
        
        let req = convert_request(request);
        
        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("HTTP-Referer", "https://rust-hermes-agent")
            .header("X-Title", "Rust Hermes Agent")
            .json(&req)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::Api(format!("HTTP {}: {}", status, body)));
        }

        let or_response: OpenRouterResponse = response.json().await?;
        convert_response(or_response)
    }

    fn estimate_tokens(&self, text: &str, _model: &ModelId) -> usize {
        text.len() / 4
    }

    fn context_length(&self, model: &ModelId) -> Option<usize> {
        // 根据模型返回上下文窗口大小
        match model.model.as_str() {
            m if m.contains("gpt-4o") || m.contains("gpt-4-turbo") => Some(128_000),
            m if m.contains("claude-3.5") => Some(200_000),
            m if m.contains("gemini") => Some(128_000),
            m if m.contains("llama-3-70b") => Some(128_000),
            _ => Some(128_000),
        }
    }
}

fn convert_request(request: ChatRequest) -> OpenRouterRequest {
    let messages = convert_messages(&request.messages);
    
    // 处理系统提示词
    let mut all_messages = Vec::new();
    if let Some(sys) = request.system_prompt {
        all_messages.push(OpenAiMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    all_messages.extend(messages);

    OpenRouterRequest {
        model: request.model.model.clone(),
        messages: all_messages,
        tools: request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| OpenAiTool {
                    tool_type: "function".to_string(),
                    function: OpenAiFunction {
                        name: t.name,
                        description: t.description,
                        parameters: t.parameters,
                    },
                })
                .collect()
        }),
        temperature: request.temperature,
        max_tokens: request.max_tokens,
        stream: false,
    }
}

fn convert_response(response: OpenRouterResponse) -> Result<ChatResponse, ProviderError> {
    let choice = response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Api("No choices in response".into()))?;

    let finish_reason = match choice.finish_reason.as_deref() {
        Some("stop") => hermes_core::FinishReason::Stop,
        Some("length") => hermes_core::FinishReason::Length,
        _ => hermes_core::FinishReason::Other,
    };

    Ok(ChatResponse {
        content: choice.message.content,
        finish_reason,
        tool_calls: convert_tool_calls(choice.message.tool_calls),
        reasoning: None,
        usage: response.usage.map(|u| hermes_core::Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        }),
    })
}
```

- [ ] **Step 2: 运行编译测试**

```bash
cargo build -p hermes-provider
```

Expected: 编译成功

- [ ] **Step 3: 添加单元测试**

在 `openrouter.rs` 末尾添加:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_models() {
        let provider = OpenRouterProvider::new("test-key");
        let models = provider.supported_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.provider == "openrouter"));
    }

    #[test]
    fn test_context_length() {
        let provider = OpenRouterProvider::new("test-key");
        let model = ModelId::new("openrouter", "openai/gpt-4o");
        assert_eq!(provider.context_length(&model), Some(128_000));
    }
}
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p hermes-provider -- openrouter
```

Expected: 所有测试通过

- [ ] **Step 5: 更新 lib.rs**

```rust
pub mod openrouter;
pub use openrouter::OpenRouterProvider;
```

- [ ] **Step 6: 提交**

```bash
git add crates/hermes-provider/src/openrouter.rs crates/hermes-provider/src/lib.rs
git commit -m "feat: 添加 OpenRouter Provider

支持 200+ 模型，包括 GPT-4、Claude、Gemini、Llama 等
API: openrouter.ai

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: GLM Provider

**Files:**
- Create: `crates/hermes-provider/src/glm.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

> GLM (智谱 AI) 是 OpenAI 兼容 API，主要差异:
> - Base URL: `https://open.bigmodel.cn/api/paas/v4`
> - 认证: API Key 在 header

- [ ] **Step 1-6: 实现 GLM Provider**

参考 openrouter.rs，差异点:
- Base URL: `https://open.bigmodel.cn/api/paas/v4/chat/completions`
- Header: `Authorization: Bearer {api_key}`
- 支持模型: glm-4, glm-4-plus, glm-3-turbo

```rust
// 核心差异
pub struct GlmProvider {
    client: Client,
    api_key: String,
}

impl GlmProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key: api_key.into(),
        }
    }
}

#[async_trait]
impl hermes_core::LlmProvider for GlmProvider {
    fn name(&self) -> &str {
        "glm"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        vec![
            ModelId::new("glm", "glm-4-plus"),
            ModelId::new("glm", "glm-4"),
            ModelId::new("glm", "glm-4-flash"),
            ModelId::new("glm", "glm-3-turbo"),
        ]
    }

    fn context_length(&self, model: &ModelId) -> Option<usize> {
        match model.model.as_str() {
            m if m.contains("glm-4") => Some(128_000),
            m if m.contains("glm-3") => Some(32_000),
            _ => Some(128_000),
        }
    }
}
```

---

## Task 3: MiniMax Provider

**Files:**
- Create: `crates/hermes-provider/src/minimax.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

> MiniMax API: `https://api.minimax.chat/v1`

- [ ] **Step 1-6: 实现 MiniMax Provider**

参考 openrouter.rs，差异点:
- Base URL: `https://api.minimax.chat/v1`
- Header: `Authorization: Bearer {api_key}`
- 支持模型: MiniMax-Text-01, hailuo-02

---

## Task 4: Qwen Provider (阿里云百炼)

**Files:**
- Create: `crates/hermes-provider/src/qwen.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

> Qwen API: `https://dashscope.aliyuncs.com/api/v1` 或通过阿里云百炼服务

- [ ] **Step 1-6: 实现 Qwen Provider**

参考 openrouter.rs，差异点:
- Base URL: `https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation`
- Header: `Authorization: Bearer {api_key}`
- 或使用阿里云 SDK 认证
- 支持模型: qwen-turbo, qwen-plus, qwen-max, qwen-long

---

## Task 5: Kimi Provider (Moonshot)

**Files:**
- Create: `crates/hermes-provider/src/kimi.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

> Kimi API: `https://api.moonshot.cn/v1`

- [ ] **Step 1-6: 实现 Kimi Provider**

参考 openrouter.rs，差异点:
- Base URL: `https://api.moonshot.cn/v1/chat/completions`
- Header: `Authorization: Bearer {api_key}`
- 支持模型: moonshot-v1-8k, moonshot-v1-32k, moonshot-v1-128k

---

## Task 6: DeepSeek Provider

**Files:**
- Create: `crates/hermes-provider/src/deepseek.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

> DeepSeek API: `https://api.deepseek.com/v1`

- [ ] **Step 1-6: 实现 DeepSeek Provider**

参考 openrouter.rs，差异点:
- Base URL: `https://api.deepseek.com/v1/chat/completions`
- Header: `Authorization: Bearer {api_key}`
- 支持模型: deepseek-chat, deepseek-coder

---

## Task 7: Provider 路由

**Files:**
- Create: `crates/hermes-provider/src/router.rs`
- Modify: `crates/hermes-provider/src/lib.rs`

实现根据 `model.provider` 自动选择 Provider 的功能:

```rust
//! Provider 路由
//!
//! 根据 model.provider 自动选择对应的 Provider

use hermes_core::{ChatRequest, ChatResponse, LlmProvider, ModelId, ProviderError};
use std::collections::HashMap;
use std::sync::Arc;

/// Provider 路由器
///
/// 根据 ModelId.provider 自动路由到对应的 Provider
pub struct ProviderRouter {
    providers: HashMap<String, Arc<dyn LlmProvider>>,
}

impl ProviderRouter {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register<P: LlmProvider + 'static>(&mut self, provider: P) {
        let name = provider.name().to_string();
        self.providers.insert(name, Arc::new(provider));
    }

    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let provider = self.providers.get(&request.model.provider)
            .ok_or_else(|| ProviderError::Api(format!("Unknown provider: {}", request.model.provider)))?;
        provider.chat(request).await
    }
}
```

---

## 验收清单

- [ ] OpenRouter Provider - 编译通过，测试通过
- [ ] GLM Provider - 编译通过，测试通过
- [ ] MiniMax Provider - 编译通过，测试通过
- [ ] Qwen Provider - 编译通过，测试通过
- [ ] Kimi Provider - 编译通过，测试通过
- [ ] DeepSeek Provider - 编译通过，测试通过
- [ ] Provider 路由 - 编译通过，测试通过
- [ ] `cargo check --all` 通过

---

## 关键文件

| 文件 | 职责 |
|------|------|
| `crates/hermes-provider/src/lib.rs` | 导出所有 Provider |
| `crates/hermes-provider/src/openai.rs` | OpenAI Provider (已有) |
| `crates/hermes-provider/src/anthropic.rs` | Anthropic Provider (已有) |
| `crates/hermes-provider/src/openrouter.rs` | OpenRouter Provider (新增) |
| `crates/hermes-provider/src/glm.rs` | GLM Provider (新增) |
| `crates/hermes-provider/src/minimax.rs` | MiniMax Provider (新增) |
| `crates/hermes-provider/src/qwen.rs` | Qwen Provider (新增) |
| `crates/hermes-provider/src/kimi.rs` | Kimi Provider (新增) |
| `crates/hermes-provider/src/deepseek.rs` | DeepSeek Provider (新增) |
| `crates/hermes-provider/src/router.rs` | Provider 路由 (新增) |

---

## 下一步

Phase 1 完成后，进入 Phase 2: Core Features (Memory, FTS5, Context Compression)
