# Infrastructure Features Design

> **For agentic workers:** Implementation using superpowers:subagent-driven-development

**Goal:** 实现 Prompt Caching、Error Classifier、Auxiliary Client 三个基础设施模块

**Architecture:** Prompt Caching 和 Error Classifier 放入 hermes-core，Auxiliary Client 独立为 hermes-auxiliary crate

**Tech Stack:** Rust, tokio, async-trait

---

## 1. Overview

三个独立但互补的基础设施模块：

1. **Prompt Caching** — 对 Anthropic 和 OpenAI 的消息数组应用缓存标记，减少 API 成本
2. **Error Classifier** — 集中式 API 错误分类，输出 FailoverReason + 恢复建议
3. **Auxiliary Client** — 多 Provider 解析链，自动故障转移，统一 `call_llm()` 入口

### 1.1 Python Reference

- `agent/prompt_caching.py` (73 lines)
- `agent/error_classifier.py` (821 lines)
- `agent/auxiliary_client.py` (2614 lines)

### 1.2 Rust Current State

- `crates/hermes-provider/` — 已有 Anthropic、OpenAI、OpenRouter、DeepSeek、GLM、Kimi、MiniMax、Qwen 等 provider 实现
- `crates/hermes-core/src/provider.rs` — `LlmProvider` trait 定义了 `chat()` 和 `chat_streaming()`
- `crates/hermes-core/src/error.rs` — `ProviderError` 枚举有 `Api`, `Auth`, `RateLimit`, `ContextTooLarge`, `InvalidModel`, `Network`
- `crates/hermes-core/src/credentials.rs` — `CredentialPool` 已存在

缺失：
- Prompt caching 策略（Anthropic + OpenAI）
- 错误分类管道（FailoverReason 枚举 + 恢复建议）
- Auxiliary client 多 provider 解析链 + 故障转移

---

## 2. Module Architecture

```
hermes-core/src/
├── prompt_caching.rs        # 新增：Prompt Caching 策略
├── error_classifier.rs      # 新增：Error Classifier

hermes-auxiliary/             # 新 crate
├── Cargo.toml
└── src/
    ├── lib.rs               # 入口：call_llm(), resolve_provider_client()
    ├── resolver.rs          # Provider 解析链
    ├── client_cache.rs      # 客户端缓存
    ├── fallback.rs          # 故障转移逻辑
    └── adapters/
        ├── mod.rs           # ClientAdapter trait
        ├── openai.rs        # OpenAI 适配（直通）
        └── anthropic.rs     # Anthropic 适配（Messages API → chat.completions）
```

---

## 3. Prompt Caching

### 3.1 CacheStrategy Trait

```rust
// prompt_caching.rs

/// 缓存策略结果
pub struct CacheResult {
    pub breakpoint_count: usize,
    pub applied: bool,
}

/// 缓存策略 trait
pub trait CacheStrategy: Send + Sync {
    /// 策略名称
    fn name(&self) -> &str;

    /// 将缓存标记应用到消息数组（原地修改或返回新数组）
    fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult;

    /// 是否对该模型启用
    fn supports_model(&self, model: &ModelId) -> bool;
}
```

### 3.2 AnthropicCache

- **策略：** 最多 4 个 `cache_control` 断点
  - 断点 1：system prompt（第一条 role=system 的消息）
  - 断点 2-4：最后 3 条非 system 消息
- **缓存 TTL：** 默认 ephemeral（5 分钟），可选 `"1h"`
- **格式：** 在消息 content 上添加 `{"type": "ephemeral"}` 标记
  - tool 消息：标记在顶层
  - string content：包装为 `[{"type": "text", "text": ..., "cache_control": ...}]`
  - list content：标记在最后一个 block

```rust
pub struct AnthropicCache {
    ttl: CacheTTL,
}

pub enum CacheTTL {
    Ephemeral,   // 5 minutes (default)
    OneHour,     // 1 hour
}
```

### 3.3 OpenAICache

- **策略：** 对符合条件的消息自动设置 `prompt_cache_key`
  - 所有 system 消息作为缓存锚点
  - 所有 tool result 消息标记为可缓存
- **要求：** 缓存的 system/tool 消息必须是连续的
- **格式：** 在 message 上设置 `prompt_cache_key` 字段（OpenAI 协议扩展）

```rust
pub struct OpenAiCache;
```

### 3.4 CacheDispatcher

```rust
/// 根据 model provider 自动选择缓存策略
pub struct CacheDispatcher {
    strategies: Vec<Box<dyn CacheStrategy>>,
}

impl CacheDispatcher {
    pub fn new() -> Self;
    pub fn with_strategy(mut self, strategy: Box<dyn CacheStrategy>) -> Self;
    pub fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult;
}
```

---

## 4. Error Classifier

### 4.1 FailoverReason Enum

```rust
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
```

### 4.2 ClassifiedError Struct

```rust
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
    pub fn is_auth(&self) -> bool;
}
```

### 4.3 Classification Pipeline

7 级优先级分类管道：

```
1. Provider-specific patterns (Anthropic thinking signature, long-context tier gate)
2. HTTP status code + message-aware refinement
3. Error code classification (from response body)
4. Message pattern matching (billing vs rate_limit vs context vs auth)
5. Server disconnect + large session → context overflow
6. Transport error heuristics
7. Fallback: unknown (retryable)
```

### 4.4 Pattern Constants

```rust
const BILLING_PATTERNS: &[&str] = &[
    "insufficient credits", "insufficient_quota", "credit balance",
    "credits have been exhausted", "top up your credits",
    "payment required", "billing hard limit",
    "exceeded your current quota", "account is deactivated",
    "plan does not include",
];

const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit", "rate_limit", "too many requests", "throttled",
    "requests per minute", "tokens per minute", "requests per day",
    "try again in", "please retry after", "resource_exhausted",
    "rate increased too quickly",
];

const CONTEXT_OVERFLOW_PATTERNS: &[&str] = &[
    "context length", "context_length_exceeded", "context window",
    "maximum context", "max context", "token limit", "reduce the length",
    "too long", "too many tokens", "beyond the maximum",
    "超过最大长度", "超出上下文", "上下文长度",
    "input length", "input is too long", "input too long",
    "request too large", "prompt too long", "max_tokens",
    "context size", "reduce your prompt",
];

const AUTH_PATTERNS: &[&str] = &[
    "invalid api key", "invalid x-api-key", "incorrect api key",
    "invalid key", "unauthorized", "authentication",
    "not authenticated", "bad credentials",
    "no api key provided",
];
```

### 4.5 Public API

```rust
/// 分类 API 错误
pub fn classify_api_error(
    error: &ProviderError,
    provider: Option<&str>,
    model: Option<&str>,
    approx_tokens: usize,
    context_length: usize,
) -> ClassifiedError;

/// 从 HTTP 响应分类
pub fn classify_http_error(
    status_code: u16,
    error_body: &str,
    provider: Option<&str>,
    model: Option<&str>,
) -> ClassifiedError;
```

---

## 5. Auxiliary Client

### 5.1 Crate Structure

```
hermes-auxiliary/          # 新 crate
├── Cargo.toml
└── src/
    ├── lib.rs             # call_llm(), resolve_provider_client()
    ├── resolver.rs        # Provider 解析链（6 级优先级）
    ├── client_cache.rs    # 客户端缓存
    ├── fallback.rs        # 故障转移逻辑
    └── adapters/
        ├── mod.rs         # ClientAdapter trait
        ├── openai.rs      # OpenAI 适配（兼容协议直通）
        └── anthropic.rs   # Anthropic 适配（Messages → chat.completions）
```

### 5.2 ClientAdapter Trait

```rust
#[async_trait]
pub trait ClientAdapter: Send + Sync {
    fn provider_name(&self) -> &str;
    fn supported_models(&self) -> &[ModelId];
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;
}
```

### 5.3 Provider Resolution Chain

6 级优先级：

```rust
pub enum ProviderChain {
    OpenRouter,      // 1. openrouter provider
    Anthropic,       // 2. native Anthropic API  
    CustomEndpoint,  // 3. OPENAI_BASE_URL / custom config
    Codex,           // 4. Codex OAuth (future)
    ApiKeyProviders, // 5. 按 API key 遍历已配置的 provider
    None,            // 6. 无可用 provider
}

pub async fn resolve_provider_client(
    provider: Option<&str>,
    model: Option<&str>,
    config: &AuxiliaryConfig,
) -> Result<(Box<dyn ClientAdapter>, ModelId)>;
```

### 5.4 Client Cache

```rust
pub struct ClientCache {
    clients: HashMap<(String, String), Box<dyn ClientAdapter>>,
}

impl ClientCache {
    pub fn get_or_create(
        &mut self,
        provider: &str,
        model: &str,
        config: &AuxiliaryConfig,
    ) -> Result<&Box<dyn ClientAdapter>>;
}
```

### 5.5 Fallback Logic

```rust
/// 支付/连接错误时尝试下一个 provider
pub async fn try_payment_fallback(
    task: &str,
    failed_provider: &str,
    error: &ProviderError,
    config: &AuxiliaryConfig,
) -> Result<Box<dyn ClientAdapter>>;
```

### 5.6 Main Public API

```rust
/// 同步 LLM 调用（统一入口）
pub async fn call_llm(
    request: ChatRequest,
    config: &AuxiliaryConfig,
) -> Result<ChatResponse, ProviderError>;

/// 获取文本类 auxiliary 客户端
pub async fn get_text_auxiliary_client(
    task: &str,
    config: &AuxiliaryConfig,
) -> Result<(Box<dyn ClientAdapter>, ModelId)>;

/// 关闭所有缓存的客户端
pub fn shutdown_cached_clients();
```

### 5.7 AuxiliaryConfig

```rust
#[derive(Debug, Clone)]
pub struct AuxiliaryConfig {
    pub default_provider: String,
    pub default_model: Option<String>,
    pub timeout_secs: f64,
    pub task_configs: HashMap<String, AuxiliaryTaskConfig>,
}

#[derive(Debug, Clone)]
pub struct AuxiliaryTaskConfig {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Option<f64>,
}
```

---

## 6. File Structure Summary

```
crates/hermes-core/src/
├── prompt_caching.rs       # 新增 (~200 lines)
├── error_classifier.rs     # 新增 (~500 lines)
└── lib.rs                  # 修改：export 新模块

crates/hermes-auxiliary/     # 新 crate (~1500 lines)
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── resolver.rs
    ├── client_cache.rs
    ├── fallback.rs
    └── adapters/
        ├── mod.rs
        ├── openai.rs
        └── anthropic.rs
```

---

## 7. Dependencies

```toml
# hermes-auxiliary/Cargo.toml
[dependencies]
hermes-core.workspace = true
hermes-provider.workspace = true
tokio.workspace = true
async-trait.workspace = true
tracing.workspace = true
serde.workspace = true
```

---

## 8. Implementation Phases

### Phase 1: Prompt Caching (P0)
- Create `crates/hermes-core/src/prompt_caching.rs`
- Implement `CacheStrategy` trait
- Implement `AnthropicCache` 
- Implement `OpenAiCache`
- Implement `CacheDispatcher`
- Add unit tests

### Phase 2: Error Classifier (P0)
- Create `crates/hermes-core/src/error_classifier.rs`
- Define `FailoverReason` enum
- Define `ClassifiedError` struct
- Implement `classify_api_error()` pipeline
- Add pattern constants
- Add unit tests

### Phase 3: hermes-auxiliary Crate (P0)
- Create crate structure
- Implement `ClientAdapter` trait
- Implement OpenAI adapter
- Implement Anthropic adapter
- Implement `ClientCache`

### Phase 4: Provider Resolver (P1)
- Implement `resolver.rs` with provider chain
- Implement `fallback.rs` with payment/connection fallback
- Wire up `call_llm()` entry point

### Phase 5: Integration Tests (P1)
- Test prompt caching with real message arrays
- Test error classification with real error patterns
- Test provider resolution and fallback
- Test client cache behavior

---

## 9. Testing Strategy

- **Prompt Caching:** Unit tests verify breakpoint placement for both Anthropic and OpenAI strategies
- **Error Classifier:** Table-driven tests mapping error messages → expected FailoverReason
- **Auxiliary Client:** Mock LLM provider for resolver logic + fallback path coverage
- **Integration:** End-to-end call_llm with mock provider chain
