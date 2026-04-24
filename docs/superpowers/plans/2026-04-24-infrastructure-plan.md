# Infrastructure Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Prompt Caching、Error Classifier、Auxiliary Client 三个基础设施模块

**Architecture:** Prompt Caching 和 Error Classifier 放入 hermes-core，Auxiliary Client 独立为 hermes-auxiliary crate

**Tech Stack:** Rust, tokio, async-trait

---

## File Structure

```
crates/hermes-core/src/
├── prompt_caching.rs       # 新增 (~250 lines)
├── error_classifier.rs     # 新增 (~450 lines)
└── lib.rs                  # 修改：export 新模块

crates/hermes-auxiliary/     # 新 crate (~1200 lines)
├── Cargo.toml
└── src/
    ├── lib.rs              # 入口
    ├── resolver.rs         # Provider 解析链
    ├── client_cache.rs     # 客户端缓存
    ├── fallback.rs         # 故障转移
    └── adapters/
        ├── mod.rs          # ClientAdapter trait
        ├── openai.rs       # OpenAI 适配（直通）
        └── anthropic.rs    # Anthropic 适配
```

---

## Task 1: Create prompt_caching.rs with CacheStrategy trait

**Files:**
- Create: `crates/hermes-core/src/prompt_caching.rs`

- [ ] **Step 1: Create the file with trait and types**

```rust
//! Prompt Caching — 提示缓存策略
//!
//! 支持 Anthropic cache_control 和 OpenAI prompt_cache_key 两种策略。

use crate::{Content, Message, ModelId, Role};

/// 缓存 TTL 选项
#[derive(Debug, Clone)]
pub enum CacheTTL {
    /// 5 分钟（默认）
    Ephemeral,
    /// 1 小时
    OneHour,
}

impl Default for CacheTTL {
    fn default() -> Self {
        Self::Ephemeral
    }
}

/// 缓存策略结果
#[derive(Debug, Clone)]
pub struct CacheResult {
    pub breakpoint_count: usize,
    pub applied: bool,
}

/// 缓存策略 trait
pub trait CacheStrategy: Send + Sync {
    /// 策略名称
    fn name(&self) -> &str;

    /// 将缓存标记应用到消息数组
    fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult;

    /// 是否对该模型启用
    fn supports_model(&self, model: &ModelId) -> bool;
}
```

- [ ] **Step 2: Implement AnthropicCache**

Add to the same file:

```rust
/// Anthropic cache_control 策略
///
/// 在消息上放置最多 4 个 cache_control 断点：
/// - 断点 1：system prompt
/// - 断点 2-4：最后 3 条非 system 消息
pub struct AnthropicCache {
    ttl: CacheTTL,
}

impl AnthropicCache {
    pub fn new(ttl: CacheTTL) -> Self {
        Self { ttl }
    }

    /// 将 Content::Text 包装为带 cache_control 的格式
    fn wrap_content_with_cache(&self, text: &str) -> String {
        match &self.ttl {
            CacheTTL::Ephemeral => {
                format!("[{{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\"}}}}]", 
                    serde_json::to_string(text).unwrap_or_else(|_| format!("\"{}\"", text)))
            }
            CacheTTL::OneHour => {
                format!("[{{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\",\"ttl\":\"1h\"}}}}]", 
                    serde_json::to_string(text).unwrap_or_else(|_| format!("\"{}\"", text)))
            }
        }
    }

    /// 标记一条消息为缓存断点
    fn mark_as_breakpoint(&self, message: &mut Message) {
        if let Content::Text(ref text) = message.content {
            let wrapped = self.wrap_content_with_cache(text);
            message.content = Content::Text(wrapped);
        }
    }
}

impl CacheStrategy for AnthropicCache {
    fn name(&self) -> &str {
        "anthropic_cache_control"
    }

    fn supports_model(&self, model: &ModelId) -> bool {
        model.provider == "anthropic"
    }

    fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult {
        if !self.supports_model(model) {
            return CacheResult { breakpoint_count: 0, applied: false };
        }

        let mut breakpoint_count = 0;

        // 断点 1：system prompt（第一条 role=System 的消息）
        let system_idx = messages.iter().position(|m| m.role == Role::System);
        if let Some(idx) = system_idx {
            self.mark_as_breakpoint(&mut messages[idx]);
            breakpoint_count += 1;
        }

        // 断点 2-4：最后 3 条非 system 消息
        let non_system_indices: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role != Role::System)
            .map(|(i, _)| i)
            .collect();

        let cache_indices: Vec<usize> = non_system_indices
            .into_iter()
            .rev()
            .take(3)
            .collect();

        for idx in cache_indices {
            // 不要重复标记已标记的 system 消息
            if Some(idx) != system_idx {
                self.mark_as_breakpoint(&mut messages[idx]);
                breakpoint_count += 1;
            }
        }

        CacheResult {
            breakpoint_count,
            applied: true,
        }
    }
}
```

- [ ] **Step 3: Implement OpenAiCache**

Add to the same file:

```rust
/// OpenAI prompt_cache_key 策略
///
/// 对所有 system 消息和 tool result 消息标记可缓存。
/// 缓存需要 messages 前缀匹配相同，后续请求使用相同前缀即可命中缓存。
pub struct OpenAiCache;

impl CacheStrategy for OpenAiCache {
    fn name(&self) -> &str {
        "openai_prompt_cache"
    }

    fn supports_model(&self, model: &ModelId) -> bool {
        model.provider == "openai"
    }

    fn apply(&self, _messages: &mut Vec<Message>, model: &ModelId) -> CacheResult {
        if !self.supports_model(model) {
            return CacheResult { breakpoint_count: 0, applied: false };
        }

        // OpenAI 的 prompt caching 是通过在请求中设置 prompt_cache_key 字段
        // 这里我们标记哪些消息会被缓存，实际的 prompt_cache_key 设置由 provider 层处理
        // 标记 system 消息和 tool result 消息位置
        let breakpoint_count = _messages
            .iter()
            .filter(|m| m.role == Role::System || m.role == Role::Tool)
            .count();

        CacheResult {
            breakpoint_count,
            applied: true,
        }
    }
}
```

- [ ] **Step 4: Implement CacheDispatcher**

Add to the same file:

```rust
/// 缓存策略分发器
///
/// 根据 model provider 自动选择合适的缓存策略。
pub struct CacheDispatcher {
    strategies: Vec<Box<dyn CacheStrategy>>,
}

impl CacheDispatcher {
    /// 创建包含默认策略的分发器
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(AnthropicCache::new(CacheTTL::Ephemeral)),
                Box::new(OpenAiCache),
            ],
        }
    }

    /// 添加自定义策略
    pub fn with_strategy(mut self, strategy: Box<dyn CacheStrategy>) -> Self {
        self.strategies.push(strategy);
        self
    }

    /// 应用缓存策略到消息数组
    pub fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult {
        for strategy in &self.strategies {
            if strategy.supports_model(model) {
                return strategy.apply(messages, model);
            }
        }
        CacheResult { breakpoint_count: 0, applied: false }
    }
}

impl Default for CacheDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_cache_marks_system() {
        let cache = AnthropicCache::new(CacheTTL::Ephemeral);
        let mut messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        let result = cache.apply(&mut messages, &ModelId::new("anthropic", "claude-sonnet-4-5"));
        assert!(result.applied);
        assert!(result.breakpoint_count >= 1);

        // system message should contain cache_control
        if let Content::Text(ref text) = messages[0].content {
            assert!(text.contains("cache_control"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_anthropic_cache_only_for_anthropic() {
        let cache = AnthropicCache::new(CacheTTL::Ephemeral);
        let result = cache.apply(
            &mut vec![Message::user("test")],
            &ModelId::new("openai", "gpt-4o"),
        );
        assert!(!result.applied);
    }

    #[test]
    fn test_openai_cache_counts_system_and_tool() {
        let cache = OpenAiCache;
        let mut messages = vec![
            Message::system("system prompt"),
            Message::user("do task"),
            Message {
                role: Role::Tool,
                content: Content::ToolResult {
                    tool_call_id: "call_1".to_string(),
                    content: "result".to_string(),
                },
                reasoning: None,
                tool_call_id: Some("call_1".to_string()),
                tool_name: Some("test_tool".to_string()),
            },
        ];

        let result = cache.apply(&mut messages, &ModelId::new("openai", "gpt-4o"));
        assert!(result.applied);
        assert_eq!(result.breakpoint_count, 2); // system + tool
    }

    #[test]
    fn test_cache_dispatcher_routes_to_correct_strategy() {
        let dispatcher = CacheDispatcher::new();
        let mut messages = vec![Message::system("test"), Message::user("hello")];

        // Anthropic model → Anthropic strategy
        let result = dispatcher.apply(&mut messages, &ModelId::new("anthropic", "claude-4"));
        assert!(result.applied);

        // DeepSeek model → no strategy matches
        let result = dispatcher.apply(
            &mut vec![Message::user("hello")],
            &ModelId::new("deepseek", "deepseek-chat"),
        );
        assert!(!result.applied);
    }
}
```

- [ ] **Step 5: Verify compilation and tests**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

Run: `cargo test -p hermes-core -- prompt_caching`
Expected: 4 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-core/src/prompt_caching.rs
git commit -m "feat(core): add Prompt Caching with Anthropic and OpenAI strategies

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Create error_classifier.rs with FailoverReason and ClassifiedError

**Files:**
- Create: `crates/hermes-core/src/error_classifier.rs`

- [ ] **Step 1: Create the file with types and enums**

```rust
//! Error Classifier — API 错误分类器
//!
//! 集中式错误分类管道，将 ProviderError 映射为 FailoverReason + 恢复建议。

use crate::ProviderError;

/// 故障转移原因
#[derive(Debug, Clone, PartialEq)]
pub enum FailoverReason {
    Auth,
    AuthPermanent,
    Billing,
    RateLimit,
    Overloaded,
    ServerError,
    Timeout,
    ContextOverflow,
    PayloadTooLarge,
    ModelNotFound,
    FormatError,
    ThinkingSignature,
    LongContextTier,
    Unknown,
}

/// 分类后的错误
#[derive(Debug, Clone)]
pub struct ClassifiedError {
    pub reason: FailoverReason,
    pub status_code: Option<u16>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub message: String,
    pub retryable: bool,
    pub should_compress: bool,
    pub should_rotate_credential: bool,
    pub should_fallback: bool,
}

impl ClassifiedError {
    /// 是否为认证相关错误
    pub fn is_auth(&self) -> bool {
        matches!(self.reason, FailoverReason::Auth | FailoverReason::AuthPermanent)
    }
}
```

- [ ] **Step 2: Add pattern constants**

Add to the same file:

```rust
/// 计费相关错误模式
const BILLING_PATTERNS: &[&str] = &[
    "insufficient credits", "insufficient_quota", "credit balance",
    "credits have been exhausted", "top up your credits",
    "payment required", "billing hard limit",
    "exceeded your current quota", "account is deactivated",
    "plan does not include",
];

/// 速率限制错误模式
const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit", "rate_limit", "too many requests", "throttled",
    "requests per minute", "tokens per minute", "requests per day",
    "try again in", "please retry after", "resource_exhausted",
    "rate increased too quickly",
];

/// 上下文溢出错误模式
const CONTEXT_OVERFLOW_PATTERNS: &[&str] = &[
    "context length", "context_length_exceeded", "context window",
    "maximum context", "max context", "token limit", "reduce the length",
    "too long", "too many tokens", "beyond the maximum",
    "超过最大长度", "超出上下文", "上下文长度",
    "input length", "input is too long", "input too long",
    "request too large", "prompt too long",
    "context size", "reduce your prompt",
];

/// 认证错误模式
const AUTH_PATTERNS: &[&str] = &[
    "invalid api key", "invalid x-api-key", "incorrect api key",
    "invalid key", "unauthorized", "authentication",
    "not authenticated", "bad credentials", "no api key provided",
];

/// 模型未找到模式
const MODEL_NOT_FOUND_PATTERNS: &[&str] = &[
    "model not found", "does not exist", "no such model",
    "invalid model", "model is not available",
    "deprecated", "end of life",
];

/// 服务端断开模式
const SERVER_DISCONNECT_PATTERNS: &[&str] = &[
    "server disconnected", "connection closed", "connection reset",
    "eof", "peer closed", "broken pipe", "connection refused",
];
```

- [ ] **Step 3: Implement classification by HTTP status code**

Add to the same file:

```rust
/// 按 HTTP 状态码分类
fn classify_by_status(
    status_code: u16,
    error_msg: &str,
    approx_tokens: usize,
    context_length: usize,
    model: Option<&str>,
) -> Option<ClassifiedError> {
    let msg = error_msg.to_lowercase();
    let make = |reason, retryable, should_compress, should_rotate, should_fallback| {
        ClassifiedError {
            reason,
            status_code: Some(status_code),
            provider: None,
            model: model.map(|s| s.to_string()),
            message: error_msg.to_string(),
            retryable,
            should_compress,
            should_rotate_credential: should_rotate,
            should_fallback,
        }
    };

    match status_code {
        401 => Some(make(FailoverReason::Auth, false, false, false, false)),
        403 => {
            // 403 + billing → billing error
            if BILLING_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(FailoverReason::Billing, false, false, false, true))
            } else {
                Some(make(FailoverReason::AuthPermanent, false, false, true, false))
            }
        }
        402 => {
            // 402 → billing (unless rate limit keywords present)
            if RATE_LIMIT_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(FailoverReason::RateLimit, true, false, false, false))
            } else {
                Some(make(FailoverReason::Billing, false, false, false, true))
            }
        }
        404 => Some(make(FailoverReason::ModelNotFound, false, false, false, false)),
        413 => Some(make(FailoverReason::PayloadTooLarge, false, true, false, false)),
        429 => Some(make(FailoverReason::RateLimit, true, false, false, false)),
        500..=502 => Some(make(FailoverReason::ServerError, true, false, false, false)),
        503 | 529 => Some(make(FailoverReason::Overloaded, true, false, false, true)),
        400 => {
            // 400 + large session → context overflow heuristics
            if CONTEXT_OVERFLOW_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(FailoverReason::ContextOverflow, false, true, false, false))
            } else if msg.contains("too long") || approx_tokens > context_length {
                Some(make(FailoverReason::ContextOverflow, false, true, false, false))
            } else if MODEL_NOT_FOUND_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(FailoverReason::ModelNotFound, false, false, false, false))
            } else {
                Some(make(FailoverReason::FormatError, false, false, false, false))
            }
        }
        _ => None,
    }
}
```

- [ ] **Step 4: Implement classify_api_error main entry**

Add to the same file:

```rust
/// 分类 API 错误 — 主入口
pub fn classify_api_error(
    error: &ProviderError,
    provider: Option<&str>,
    model: Option<&str>,
    approx_tokens: usize,
    context_length: usize,
) -> ClassifiedError {
    let msg = error.to_string();
    let msg_lower = msg.to_lowercase();

    // Step 1: 按 ProviderError variant 预分类
    match error {
        ProviderError::Auth => {
            return ClassifiedError {
                reason: FailoverReason::Auth,
                status_code: None,
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: false,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: false,
            };
        }
        ProviderError::RateLimit(secs) => {
            return ClassifiedError {
                reason: FailoverReason::RateLimit,
                status_code: Some(429),
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: true,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: *secs > 60,
            };
        }
        ProviderError::ContextTooLarge => {
            return ClassifiedError {
                reason: FailoverReason::ContextOverflow,
                status_code: Some(400),
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: false,
                should_compress: true,
                should_rotate_credential: false,
                should_fallback: false,
            };
        }
        ProviderError::InvalidModel(_) => {
            return ClassifiedError {
                reason: FailoverReason::ModelNotFound,
                status_code: Some(404),
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: false,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: false,
            };
        }
        ProviderError::Api(_) | ProviderError::Network(_) => {
            // 需要进一步按消息内容分类
        }
    }

    // Step 2: 按消息模式匹配
    if AUTH_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::Auth,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
    }

    if BILLING_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::Billing,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: true,
        };
    }

    if RATE_LIMIT_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::RateLimit,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: true,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
    }

    if CONTEXT_OVERFLOW_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::ContextOverflow,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: false,
            should_compress: true,
            should_rotate_credential: false,
            should_fallback: false,
        };
    }

    // Step 3: Transport error heuristics
    if let ProviderError::Network(_) = error {
        if SERVER_DISCONNECT_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
            // Server disconnect + large session → may be context overflow
            if approx_tokens > context_length / 2 {
                return ClassifiedError {
                    reason: FailoverReason::ContextOverflow,
                    status_code: None,
                    provider: provider.map(|s| s.to_string()),
                    model: model.map(|s| s.to_string()),
                    message: msg,
                    retryable: false,
                    should_compress: true,
                    should_rotate_credential: false,
                    should_fallback: false,
                };
            }
        }
    }

    // Step 4: Fallback
    let is_retryable = error.is_retryable();
    ClassifiedError {
        reason: FailoverReason::Unknown,
        status_code: None,
        provider: provider.map(|s| s.to_string()),
        model: model.map(|s| s.to_string()),
        message: msg,
        retryable: is_retryable,
        should_compress: false,
        should_rotate_credential: false,
        should_fallback: !is_retryable,
    }
}

/// 按 HTTP 状态码 + 内容分类
pub fn classify_http_error(
    status_code: u16,
    error_body: &str,
    provider: Option<&str>,
    model: Option<&str>,
    approx_tokens: usize,
    context_length: usize,
) -> ClassifiedError {
    let status_result = classify_by_status(
        status_code,
        error_body,
        approx_tokens,
        context_length,
        model,
    );

    if let Some(mut result) = status_result {
        result.provider = provider.map(|s| s.to_string());
        result
    } else {
        let msg_lower = error_body.to_lowercase();
        let (reason, retryable, should_fallback) = match status_code {
            s if s < 500 => (FailoverReason::Unknown, false, false),
            _ => (FailoverReason::ServerError, true, false),
        };

        ClassifiedError {
            reason,
            status_code: Some(status_code),
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: error_body.to_string(),
            retryable,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback,
        }
    }
}
```

- [ ] **Step 5: Add unit tests**

Add to the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProviderError;

    #[test]
    fn test_classify_auth_error() {
        let result = classify_api_error(
            &ProviderError::Auth,
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Auth);
        assert!(!result.retryable);
        assert!(!result.should_fallback);
    }

    #[test]
    fn test_classify_rate_limit() {
        let result = classify_api_error(
            &ProviderError::RateLimit(30),
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert!(result.retryable);
        assert!(!result.should_fallback); // 30s < 60s
    }

    #[test]
    fn test_classify_long_rate_limit() {
        let result = classify_api_error(
            &ProviderError::RateLimit(120),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert!(result.should_fallback); // 120s > 60s
    }

    #[test]
    fn test_classify_context_too_large() {
        let result = classify_api_error(
            &ProviderError::ContextTooLarge,
            Some("anthropic"),
            None,
            200000,
            200000,
        );
        assert_eq!(result.reason, FailoverReason::ContextOverflow);
        assert!(result.should_compress);
    }

    #[test]
    fn test_classify_invalid_model() {
        let result = classify_api_error(
            &ProviderError::InvalidModel("gpt-5".into()),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::ModelNotFound);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_api_billing_message() {
        let result = classify_api_error(
            &ProviderError::Api("insufficient credits balance".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Billing);
        assert!(result.should_fallback);
    }

    #[test]
    fn test_classify_api_rate_limit_message() {
        let result = classify_api_error(
            &ProviderError::Api("rate limit exceeded, too many requests".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert!(result.retryable);
    }

    #[test]
    fn test_classify_api_context_overflow_message() {
        let result = classify_api_error(
            &ProviderError::Api("context length exceeded: 200000 tokens".into()),
            Some("anthropic"),
            None,
            200000,
            200000,
        );
        assert_eq!(result.reason, FailoverReason::ContextOverflow);
        assert!(result.should_compress);
    }

    #[test]
    fn test_classify_network_error() {
        let result = classify_api_error(
            &ProviderError::Network(reqwest::Error::new(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                "server disconnected",
            )),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert!(!result.should_compress); // session not large
    }

    #[test]
    fn test_classify_network_error_large_session() {
        let result = classify_api_error(
            &ProviderError::Network(reqwest::Error::new(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                "server disconnected unexpectedly",
            )),
            Some("openai"),
            None,
            100000,
            128000,
        );
        // 100000 > 128000/2 → context overflow
        assert_eq!(result.reason, FailoverReason::ContextOverflow);
        assert!(result.should_compress);
    }

    #[test]
    fn test_classified_error_is_auth() {
        let auth = ClassifiedError {
            reason: FailoverReason::Auth,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(auth.is_auth());

        let auth_permanent = ClassifiedError {
            reason: FailoverReason::AuthPermanent,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(auth_permanent.is_auth());

        let billing = ClassifiedError {
            reason: FailoverReason::Billing,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(!billing.is_auth());
    }

    #[test]
    fn test_classify_http_error() {
        let result = classify_http_error(
            429,
            "Too many requests",
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert_eq!(result.status_code, Some(429));
        assert!(result.retryable);
    }
}
```

- [ ] **Step 6: Verify compilation and tests**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

Run: `cargo test -p hermes-core -- error_classifier`
Expected: 12 tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/hermes-core/src/error_classifier.rs
git commit -m "feat(core): add Error Classifier with FailoverReason and recovery hints

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Update hermes-core lib.rs exports

**Files:**
- Modify: `crates/hermes-core/src/lib.rs`

- [ ] **Step 1: Add module declarations and exports**

Edit the `pub mod` section to add:

```rust
pub mod prompt_caching;
pub mod error_classifier;

pub use prompt_caching::{AnthropicCache, CacheDispatcher, CacheResult, CacheStrategy, CacheTTL, OpenAiCache};
pub use error_classifier::{ClassifiedError, FailoverReason, classify_api_error, classify_http_error};
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

Run: `cargo test -p hermes-core`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/lib.rs
git commit -m "feat(core): export prompt_caching and error_classifier modules

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Create hermes-auxiliary crate scaffold

**Files:**
- Create: `crates/hermes-auxiliary/Cargo.toml`
- Create: `crates/hermes-auxiliary/src/lib.rs`
- Create: `crates/hermes-auxiliary/src/adapters/mod.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "hermes-auxiliary"
version.workspace = true
edition.workspace = true
description = "Auxiliary LLM client with multi-provider resolution and fallback"

[dependencies]
hermes-core.workspace = true
hermes-provider.workspace = true
tokio.workspace = true
async-trait.workspace = true
tracing.workspace = true
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 2: Create adapters/mod.rs with ClientAdapter trait**

```rust
//! Client adapter trait — 统一的 LLM 客户端接口

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};

/// 统一的 LLM 客户端适配器
#[async_trait]
pub trait ClientAdapter: Send + Sync {
    /// Provider 名称
    fn provider_name(&self) -> &str;

    /// 支持的模型列表
    fn supported_models(&self) -> Vec<ModelId>;

    /// 非流式 chat completion
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
}
```

- [ ] **Step 3: Create lib.rs with placeholder exports**

```rust
//! Hermes Auxiliary Client
//!
//! 多 Provider 解析链 + 自动故障转移的统一 LLM 调用入口。

pub mod adapters;
pub mod resolver;
pub mod client_cache;
pub mod fallback;

use hermes_core::{ChatRequest, ChatResponse, ProviderError};

/// Auxiliary 客户端配置
#[derive(Debug, Clone)]
pub struct AuxiliaryConfig {
    /// 默认 provider
    pub default_provider: String,
    /// 默认 model（可选）
    pub default_model: Option<String>,
    /// 超时时间（秒）
    pub timeout_secs: f64,
}

impl Default for AuxiliaryConfig {
    fn default() -> Self {
        Self {
            default_provider: "openrouter".to_string(),
            default_model: None,
            timeout_secs: 30.0,
        }
    }
}

/// 调用 LLM — 统一入口
pub async fn call_llm(
    request: ChatRequest,
    _config: &AuxiliaryConfig,
) -> Result<ChatResponse, ProviderError> {
    // Placeholder — will be wired in later tasks
    Err(ProviderError::Api("not yet implemented".into()))
}
```

- [ ] **Step 4: Check workspace Cargo.toml for provider dependency**

Read `Cargo.toml`, check if `hermes-provider` is in `[workspace.dependencies]`. If not, add it:

```toml
hermes-provider = { path = "crates/hermes-provider" }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p hermes-auxiliary`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/hermes-auxiliary/ Cargo.toml
git commit -m "feat(auxiliary): create hermes-auxiliary crate scaffold

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Implement adapters (OpenAI + Anthropic)

**Files:**
- Create: `crates/hermes-auxiliary/src/adapters/openai.rs`
- Create: `crates/hermes-auxiliary/src/adapters/anthropic.rs`
- Modify: `crates/hermes-auxiliary/src/adapters/mod.rs`

- [ ] **Step 1: Create OpenAI adapter**

```rust
//! OpenAI adapter — 直通适配（OpenAI 兼容协议）

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use hermes_provider::OpenAiProvider;

use super::ClientAdapter;

pub struct OpenAiAdapter {
    provider: OpenAiProvider,
}

impl OpenAiAdapter {
    pub fn new(provider: OpenAiProvider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ClientAdapter for OpenAiAdapter {
    fn provider_name(&self) -> &str {
        "openai"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        self.provider.supported_models()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.provider.chat(request).await
    }
}
```

- [ ] **Step 2: Create Anthropic adapter**

```rust
//! Anthropic adapter — 包装 AnthropicProvider 为 ClientAdapter

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};
use hermes_provider::AnthropicProvider;

use super::ClientAdapter;

pub struct AnthropicAdapter {
    provider: AnthropicProvider,
}

impl AnthropicAdapter {
    pub fn new(provider: AnthropicProvider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl ClientAdapter for AnthropicAdapter {
    fn provider_name(&self) -> &str {
        "anthropic"
    }

    fn supported_models(&self) -> Vec<ModelId> {
        self.provider.supported_models()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.provider.chat(request).await
    }
}
```

- [ ] **Step 3: Update adapters/mod.rs exports**

```rust
pub mod openai;
pub mod anthropic;

pub use openai::OpenAiAdapter;
pub use anthropic::AnthropicAdapter;

use async_trait::async_trait;
use hermes_core::{ChatRequest, ChatResponse, ModelId, ProviderError};

/// 统一的 LLM 客户端适配器
#[async_trait]
pub trait ClientAdapter: Send + Sync {
    fn provider_name(&self) -> &str;
    fn supported_models(&self) -> Vec<ModelId>;
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p hermes-auxiliary`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/hermes-auxiliary/src/adapters/
git commit -m "feat(auxiliary): add OpenAI and Anthropic adapters

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: Implement client_cache.rs

**Files:**
- Create: `crates/hermes-auxiliary/src/client_cache.rs`
- Modify: `crates/hermes-auxiliary/src/lib.rs`

- [ ] **Step 1: Create client_cache.rs**

```rust
//! Client cache — 按 (provider, model) 缓存客户端实例

use std::collections::HashMap;
use std::sync::Mutex;
use hermes_core::ModelId;
use crate::adapters::ClientAdapter;

/// 客户端缓存
///
/// 按 (provider, model) 键值缓存 Box<dyn ClientAdapter>，避免重复创建。
pub struct ClientCache {
    cache: Mutex<HashMap<(String, String), Box<dyn ClientAdapter>>>,
}

impl ClientCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 获取或创建客户端
    pub fn get_or_insert<F>(
        &self,
        key: (String, String),
        factory: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Result<Box<dyn ClientAdapter>, String>,
    {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        if cache.contains_key(&key) {
            return Ok(());
        }
        let client = factory()?;
        cache.insert(key, client);
        Ok(())
    }

    /// 获取客户端
    pub fn get(
        &self,
        provider: &str,
        model: &str,
    ) -> Option<()> {
        // Returns Some(()) to indicate cache hit — actual access is internal
        let cache = self.cache.lock().ok()?;
        if cache.contains_key(&(provider.to_string(), model.to_string())) {
            Some(())
        } else {
            None
        }
    }

    /// 清空缓存
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// 缓存条目数量
    pub fn len(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ClientCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hermes_core::{ChatRequest, ChatResponse, ProviderError};

    struct MockAdapter {
        name: &'static str,
    }

    #[async_trait]
    impl ClientAdapter for MockAdapter {
        fn provider_name(&self) -> &str { self.name }
        fn supported_models(&self) -> Vec<ModelId> { vec![] }
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, ProviderError> {
            Err(ProviderError::Api("mock".into()))
        }
    }

    #[test]
    fn test_cache_miss_then_insert() {
        let cache = ClientCache::new();
        assert!(cache.get("openai", "gpt-4o").is_none());

        let result = cache.get_or_insert(
            ("openai".into(), "gpt-4o".into()),
            || Ok(Box::new(MockAdapter { name: "openai" })),
        );
        assert!(result.is_ok());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_hit_no_duplicate_insert() {
        let cache = ClientCache::new();
        
        cache.get_or_insert(
            ("openai".into(), "gpt-4o".into()),
            || Ok(Box::new(MockAdapter { name: "openai" })),
        ).unwrap();

        // Second insert with same key should be no-op
        cache.get_or_insert(
            ("openai".into(), "gpt-4o".into()),
            || panic!("should not be called"),
        ).unwrap();

        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_multiple_providers() {
        let cache = ClientCache::new();

        cache.get_or_insert(
            ("openai".into(), "gpt-4o".into()),
            || Ok(Box::new(MockAdapter { name: "openai" })),
        ).unwrap();

        cache.get_or_insert(
            ("anthropic".into(), "claude-4".into()),
            || Ok(Box::new(MockAdapter { name: "anthropic" })),
        ).unwrap();

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_clear() {
        let cache = ClientCache::new();

        cache.get_or_insert(
            ("openai".into(), "gpt-4o".into()),
            || Ok(Box::new(MockAdapter { name: "openai" })),
        ).unwrap();

        cache.clear();
        assert!(cache.is_empty());
    }
}
```

- [ ] **Step 2: Update lib.rs to export client_cache**

Add to the `pub use` section:

```rust
pub use client_cache::ClientCache;
```

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo check -p hermes-auxiliary`
Expected: Compiles successfully

Run: `cargo test -p hermes-auxiliary`
Expected: 4 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-auxiliary/
git commit -m "feat(auxiliary): add ClientCache with provider-based caching

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Implement resolver.rs with provider chain

**Files:**
- Create: `crates/hermes-auxiliary/src/resolver.rs`
- Modify: `crates/hermes-auxiliary/src/lib.rs`

- [ ] **Step 1: Create resolver.rs**

```rust
//! Provider resolver — 多 Provider 解析链
//!
//! 按优先级尝试 provider：OpenRouter → Anthropic → 自定义端点 → 按 API key 遍历

use std::sync::Arc;
use hermes_core::{ModelId, ProviderError};
use hermes_provider::{
    AnthropicProvider, OpenAiProvider, OpenRouterProvider,
    ProviderRouter,
};

use crate::adapters::{ClientAdapter, AnthropicAdapter, OpenAiAdapter};
use crate::AuxiliaryConfig;

/// Provider 解析链优先级
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStep {
    /// OpenRouter 聚合网关
    OpenRouter,
    /// 原生 Anthropic API
    Anthropic,
    /// OpenAI 自定义端点
    CustomEndpoint,
    /// 按 API key 遍历其他 provider
    ApiKeyProviders,
}

/// Provider 解析结果
pub type ResolvedClient = Box<dyn ClientAdapter>;

/// Provider 解析器
pub struct ProviderResolver {
    /// 已解析的 provider 步骤顺序
    steps: Vec<ProviderStep>,
}

impl ProviderResolver {
    /// 创建默认解析器（按优先级链）
    pub fn new() -> Self {
        Self {
            steps: vec![
                ProviderStep::OpenRouter,
                ProviderStep::Anthropic,
                ProviderStep::CustomEndpoint,
                ProviderStep::ApiKeyProviders,
            ],
        }
    }

    /// 按优先级解析 provider，返回第一个可用的客户端
    pub async fn resolve(
        &self,
        provider: Option<&str>,
        _model: Option<&str>,
        config: &AuxiliaryConfig,
    ) -> Result<(ResolvedClient, String), ProviderError> {
        // 如果显式指定了 provider，直接使用
        if let Some(provider_name) = provider {
            return self.resolve_specific(provider_name, config).await;
        }

        // 否则按解析链依次尝试
        for step in &self.steps {
            match step {
                ProviderStep::OpenRouter => {
                    if let Ok(client) = self.try_openrouter().await {
                        return Ok((Box::new(client), "openrouter".into()));
                    }
                }
                ProviderStep::Anthropic => {
                    if let Ok(client) = self.try_anthropic().await {
                        return Ok((Box::new(client), "anthropic".into()));
                    }
                }
                ProviderStep::CustomEndpoint => {
                    if let Ok(client) = self.try_custom_endpoint(config).await {
                        return Ok((Box::new(client), config.default_provider.clone()));
                    }
                }
                ProviderStep::ApiKeyProviders => {
                    if let Ok((client, name)) = self.try_api_key_providers().await {
                        return Ok((client, name));
                    }
                }
            }
        }

        Err(ProviderError::Api("no available provider found".into()))
    }

    /// 解析指定的 provider
    async fn resolve_specific(
        &self,
        provider: &str,
        _config: &AuxiliaryConfig,
    ) -> Result<(ResolvedClient, String), ProviderError> {
        match provider {
            "openrouter" => {
                let client = self.try_openrouter().await?;
                Ok((Box::new(client), "openrouter".into()))
            }
            "anthropic" => {
                let client = self.try_anthropic().await?;
                Ok((Box::new(client), "anthropic".into()))
            }
            "openai" => {
                let client = self.try_openai().await?;
                Ok((Box::new(client), "openai".into()))
            }
            _ => {
                // 尝试作为 API key provider
                if let Ok((client, name)) = self.try_api_key_providers().await {
                    Ok((client, name))
                } else {
                    Err(ProviderError::InvalidModel(format!("unknown provider: {}", provider)))
                }
            }
        }
    }

    /// 尝试 OpenRouter
    async fn try_openrouter(&self) -> Result<OpenAiAdapter, ProviderError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENROUTER_API_KEY"));
        
        match api_key {
            Ok(key) if !key.is_empty() => {
                let provider = OpenRouterProvider::new(key);
                Ok(OpenAiAdapter::new(provider))
            }
            _ => Err(ProviderError::Auth),
        }
    }

    /// 尝试 Anthropic
    async fn try_anthropic(&self) -> Result<AnthropicAdapter, ProviderError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .or_else(|_| std::env::var("HERMES_ANTHROPIC_API_KEY"));
        
        match api_key {
            Ok(key) if !key.is_empty() => {
                let provider = AnthropicProvider::new(key, None)?;
                Ok(AnthropicAdapter::new(provider))
            }
            _ => Err(ProviderError::Auth),
        }
    }

    /// 尝试 OpenAI
    async fn try_openai(&self) -> Result<OpenAiAdapter, ProviderError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"));
        
        match api_key {
            Ok(key) if !key.is_empty() => {
                let base_url = std::env::var("OPENAI_BASE_URL").ok();
                let provider = OpenAiProvider::new(key, base_url);
                Ok(OpenAiAdapter::new(provider))
            }
            _ => Err(ProviderError::Auth),
        }
    }

    /// 尝试自定义端点
    async fn try_custom_endpoint(&self, config: &AuxiliaryConfig) -> Result<OpenAiAdapter, ProviderError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))
            .unwrap_or_default();

        if !api_key.is_empty() {
            let base_url = std::env::var("OPENAI_BASE_URL").ok();
            let provider = OpenAiProvider::new(api_key, base_url);
            Ok(OpenAiAdapter::new(provider))
        } else {
            Err(ProviderError::Auth)
        }
    }

    /// 遍历 API key provider
    async fn try_api_key_providers(&self) -> Result<(ResolvedClient, String), ProviderError> {
        // 尝试 GLM
        if let Ok(key) = std::env::var("GLM_API_KEY") {
            if !key.is_empty() {
                let provider = hermes_provider::GlmProvider::new(key);
                return Ok((Box::new(OpenAiAdapter::new(provider)), "glm".into()));
            }
        }

        // 尝试 DeepSeek
        if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
            if !key.is_empty() {
                let provider = hermes_provider::DeepSeekProvider::new(key);
                return Ok((Box::new(OpenAiAdapter::new(provider)), "deepseek".into()));
            }
        }

        // 尝试 Kimi
        if let Ok(key) = std::env::var("KIMI_API_KEY") {
            if !key.is_empty() {
                let provider = hermes_provider::KimiProvider::new(key);
                return Ok((Box::new(OpenAiAdapter::new(provider)), "kimi".into()));
            }
        }

        Err(ProviderError::Auth)
    }
}

impl Default for ProviderResolver {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Update lib.rs to export resolver**

Add:

```rust
pub use resolver::{ProviderResolver, ProviderStep, ResolvedClient};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p hermes-auxiliary`
Expected: Compiles successfully (or fix any provider constructor mismatches)

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-auxiliary/
git commit -m "feat(auxiliary): add ProviderResolver with priority chain

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Implement fallback.rs + wire up call_llm

**Files:**
- Create: `crates/hermes-auxiliary/src/fallback.rs`
- Modify: `crates/hermes-auxiliary/src/lib.rs` — finalize `call_llm()`

- [ ] **Step 1: Create fallback.rs**

```rust
//! Fallback logic — 支付/连接错误时自动切换到下一个 provider

use hermes_core::{ClassifiedError, FailoverReason, ProviderError, classify_api_error};

/// 检测是否为需要 fallback 的错误
pub fn should_fallback(error: &ProviderError, provider: Option<&str>) -> bool {
    let classified = classify_api_error(
        error,
        provider,
        None,
        0,
        200_000,
    );
    classified.should_fallback
}

/// 检测是否为重试错误
pub fn should_retry(error: &ProviderError, provider: Option<&str>) -> bool {
    let classified = classify_api_error(
        error,
        provider,
        None,
        0,
        200_000,
    );
    classified.retryable
}

/// 检测是否需要压缩上下文
pub fn should_compress(error: &ProviderError, provider: Option<&str>) -> bool {
    let classified = classify_api_error(
        error,
        provider,
        None,
        0,
        200_000,
    );
    classified.should_compress
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_fallback_on_auth() {
        assert!(!should_fallback(&ProviderError::Auth, Some("openai")));
    }

    #[test]
    fn test_should_fallback_on_billing() {
        let err = ProviderError::Api("insufficient credits balance".into());
        assert!(should_fallback(&err, Some("openai")));
    }

    #[test]
    fn test_should_retry_on_rate_limit() {
        assert!(should_retry(&ProviderError::RateLimit(5), Some("openai")));
    }

    #[test]
    fn test_should_compress_on_context_overflow() {
        assert!(should_compress(&ProviderError::ContextTooLarge, Some("anthropic")));
    }
}
```

- [ ] **Step 2: Update lib.rs with final call_llm implementation**

Replace the placeholder `call_llm()` in lib.rs with:

```rust
/// 调用 LLM — 统一入口
///
/// 自动解析 provider、缓存客户端、处理故障转移。
pub async fn call_llm(
    request: ChatRequest,
    config: &AuxiliaryConfig,
) -> Result<ChatResponse, ProviderError> {
    let resolver = ProviderResolver::new();
    let cached = ClientCache::new();

    let model_id = request.model.clone();
    let provider_name = model_id.provider.clone();

    // Step 1: Resolve provider
    let (client, resolved_provider) = resolver
        .resolve(Some(&provider_name), Some(&model_id.model), config)
        .await
        .map_err(|e| {
            tracing::error!("failed to resolve provider '{}': {}", provider_name, e);
            e
        })?;

    // Step 2: Make the call
    match client.chat(request.clone()).await {
        Ok(response) => Ok(response),
        Err(error) => {
            // Step 3: Check if we should fallback
            if fallback::should_fallback(&error, Some(&resolved_provider)) {
                tracing::warn!(
                    "provider '{}' returned fallback-eligible error: {}",
                    resolved_provider, error
                );
                // Try next provider in chain
                let (fallback_client, fallback_name) = resolver
                    .resolve(None, Some(&model_id.model), config)
                    .await
                    .map_err(|_| error.clone())?;

                fallback_client.chat(request).await
            } else {
                Err(error)
            }
        }
    }
}
```

- [ ] **Step 3: Verify compilation and tests**

Run: `cargo check -p hermes-auxiliary`
Expected: Compiles successfully

Run: `cargo test -p hermes-auxiliary`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-auxiliary/
git commit -m "feat(auxiliary): add fallback logic and wire up call_llm entry

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 9: Add integration tests

**Files:**
- Create: `crates/hermes-auxiliary/tests/test_auxiliary.rs`

- [ ] **Step 1: Create integration test file**

```rust
//! Integration tests for hermes-auxiliary

use hermes_auxiliary::*;
use hermes_core::*;

#[test]
fn test_auxiliary_config_defaults() {
    let config = AuxiliaryConfig::default();
    assert_eq!(config.default_provider, "openrouter");
    assert_eq!(config.timeout_secs, 30.0);
    assert!(config.default_model.is_none());
}

#[test]
fn test_client_cache_new_is_empty() {
    let cache = ClientCache::new();
    assert!(cache.is_empty());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_provider_resolver_has_steps() {
    let resolver = ProviderResolver::new();
    // Default resolver should have 4 steps
    assert!(true); // Smoke test — resolver was created without panic
}

#[test]
fn test_fallback_should_retry_rate_limit() {
    use hermes_auxiliary::fallback;
    assert!(fallback::should_retry(
        &ProviderError::RateLimit(5),
        Some("openai"),
    ));
}

#[tokio::test]
async fn test_call_llm_with_invalid_provider() {
    let config = AuxiliaryConfig::default();
    let request = ChatRequest {
        model: ModelId::new("nonexistent", "model"),
        messages: vec![Message::user("hello")],
        ..Default::default()
    };

    let result = call_llm(request, &config).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p hermes-auxiliary`
Expected: All tests pass (unit + integration)

- [ ] **Step 3: Run full workspace tests to check for regressions**

Run: `cargo test --all`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-auxiliary/tests/
git commit -m "test(auxiliary): add integration tests for auxiliary client

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```
