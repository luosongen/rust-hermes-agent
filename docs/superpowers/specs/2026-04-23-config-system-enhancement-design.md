# Configuration System Enhancement Design

> **For agentic workers:** Implementation using superpowers:subagent-driven-development

**Goal:** Enhance rust-hermes-agent's configuration system to match Python hermes-agent's comprehensive config (20+ categories)

**Architecture:** Hybrid config architecture with provider routing, smart model selection, multi-backend execution, context compression, auxiliary models, MCP server management, STT providers, delegation config, display settings, and personality presets.

**Tech Stack:** Rust, serde, figment, tokio

---

## 1. Provider System

### 1.1 Core Provider Enum

```rust
pub enum CoreProvider {
    OpenRouter(OpenRouterConfig),
    Nous(NousConfig),
    Anthropic(AnthropicConfig),
    OpenAI(OpenAIConfig),
    Gemini(GeminiConfig),
    HuggingFace(HuggingFaceConfig),
    MiniMax(MiniMaxConfig),
}
```

### 1.2 ProviderConfig Hybrid

```rust
pub enum ProviderConfig {
    Core(CoreProvider),
    Custom { name: String, base_url: Url, api_key: Secret<String> },
}
```

### 1.3 ProviderSettings Structure

```rust
pub struct ProviderSettings {
    pub default: String,                    // "openrouter/..."
    pub priority: Vec<String>,               // ["anthropic/...", "openai/..."]
    pub fallback: String,                    // "openai/..."
    pub smart_router: SmartRouterConfig,
    pub allowed_rrt: Option<u32>,
    pub models: Vec<ProviderModelConfig>,
}
```

### 1.4 SmartRouterConfig

```rust
pub struct SmartRouterConfig {
    pub enabled: bool,
    pub cheap_model: String,      // "openrouter/..."
    pub cheap_threshold: f32,      // 0.3
    pub cheap_max_tokens: u32,     // 4096
    pub default_model: String,
}
```

### 1.5 ProviderModelConfig

```rust
pub struct ProviderModelConfig {
    pub name: String,             // "anthropic/claude-3-5-sonnet"
    pub provider: String,          // "anthropic"
    pub enabled: bool,
    pub cache_priority: i32,
    pub context_window: u32,
    pub supports_function_calls: bool,
}
```

### 1.6 NousConfig, OpenRouterConfig, etc.

Provider-specific configs with `api_key: Secret<String>`, `base_url: Option<Url>`, `models: Vec<String>`, etc.

---

## 2. Backend System

### 2.1 BackendConfig Enum

```rust
pub enum BackendConfig {
    Local(LocalBackend),
    Docker(DockerBackend),
    SSH(SSHBackend),
    Singularity(SingularityBackend),
    Modal(ModalBackend),
    Daytona(DaytonaBackend),
}
```

### 2.2 BackendSettings

```rust
pub struct BackendSettings {
    pub default: BackendConfig,
    pub execution: ExecutionConfig,
}
```

### 2.3 ExecutionConfig

```rust
pub struct ExecutionConfig {
    pub backend: String,          // "local"
    pub workdir: PathBuf,
    pub container_image: Option<String>,
    pub ssh: SSHConfig,
    pub singularity: SingularityConfig,
    pub modal: ModalConfig,
    pub daytona: DaytonaConfig,
}
```

---

## 3. Context Compression

### 3.1 CompressionConfig

```rust
pub struct CompressionConfig {
    pub enabled: bool,
    pub threshold: u32,          // 60000 tokens
    pub target_ratio: f32,       // 0.7
    pub protect_last_n: u32,      // 10 messages
    pub model: String,            // "openai/..."
}
```

---

## 4. Auxiliary Models

### 4.1 AuxiliaryConfig

```rust
pub struct AuxiliaryConfig {
    pub vision: VisionConfig,
    pub web_extract: WebExtractConfig,
}
```

### 4.2 VisionConfig

```rust
pub struct VisionConfig {
    pub provider: String,        // "openai"
    pub model: String,           // "openai/gpt-4o"
}
```

### 4.3 WebExtractConfig

```rust
pub struct WebExtractConfig {
    pub provider: String,        // "openai"
    pub model: String,           // "openai/gpt-4o-mini"
}
```

---

## 5. MCP Servers

### 5.1 McpServersConfig

```rust
pub struct McpServersConfig {
    pub servers: Vec<McpServerConfig>,
}

pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
    pub enabled: bool,
}

pub enum McpTransport {
    Stdio { command: String, args: Vec<String> },
    Http { url: Url },
}
```

---

## 6. STT (Speech-to-Text)

### 6.1 SttConfig

```rust
pub struct SttConfig {
    pub default: String,         // "local"
    pub providers: Vec<SttProviderConfig>,
}

pub struct SttProviderConfig {
    pub name: String,            // "groq"
    pub provider: String,        // "groq", "openai", "mistral", "local"
    pub model: String,           // "whisper-large-v3"
    pub api_key: Option<Secret<String>>,
    pub base_url: Option<Url>,
    pub enabled: bool,
}
```

---

## 7. Delegation Config

### 7.1 DelegationConfig

```rust
pub struct DelegationConfig {
    pub enabled: bool,
    pub default_personality: Option<String>,
    pub default_model: String,
    pub max_depth: u32,
    pub max_tokens: Option<u32>,
    pub terminate_on_model: Vec<String>,
}
```

---

## 8. Display Config

### 8.1 DisplayConfig

```rust
pub struct DisplayConfig {
    pub compact: bool,
    pub tool_progress: bool,
    pub skin: String,            // "default"
}
```

---

## 9. Personality Config

### 9.1 PersonalityConfig

```rust
pub struct PersonalityConfig {
    pub default: String,         // "helpfulness"
    pub personalities: Vec<PersonalityPreset>,
}

pub struct PersonalityPreset {
    pub name: String,
    pub system_prompt: String,
    pub model: String,
}
```

---

## 10. Config File Structure

### 10.1 Settings -> Config conversion

```rust
impl Settings {
    pub fn to_config(&self) -> Result<Config, ConfigError> {
        // Validate providers, build Config with all sections
    }
}
```

### 10.2 Config struct (top-level)

```rust
pub struct Config {
    pub providers: ProviderSettings,
    pub backends: BackendSettings,
    pub compression: CompressionConfig,
    pub auxiliary: AuxiliaryConfig,
    pub mcp_servers: McpServersConfig,
    pub stt: SttConfig,
    pub delegation: DelegationConfig,
    pub display: DisplayConfig,
    pub personality: PersonalityConfig,
}
```

---

## Implementation Phases

### Phase 1 (P0 - Foundation): Provider System

**Files:**
- Modify: `crates/hermes-core/src/config/provider.rs`
- Create: `crates/hermes-core/src/config/provider_core.rs`
- Create: `crates/hermes-core/src/config/provider_custom.rs`
- Modify: `crates/hermes-core/src/config/mod.rs`

- [ ] Define `CoreProvider` enum with all 7 providers
- [ ] Define `ProviderConfig` hybrid enum
- [ ] Define `ProviderSettings` with routing config
- [ ] Define `SmartRouterConfig` for cheap model routing
- [ ] Define `ProviderModelConfig` per-model settings
- [ ] Implement provider validation in Settings
- [ ] Add provider tests

### Phase 2 (P1 - High): Backend System

**Files:**
- Create: `crates/hermes-core/src/config/backend.rs`
- Create: `crates/hermes-core/src/config/backend_*.rs` (one per backend)
- Modify: `crates/hermes-core/src/config/mod.rs`

- [ ] Define `BackendConfig` enum with all 6 backends
- [ ] Define `BackendSettings` with default selection
- [ ] Define `ExecutionConfig` with all backend params
- [ ] Implement backend validation
- [ ] Add backend tests

### Phase 3 (P1 - High): Context Compression

**Files:**
- Create: `crates/hermes-core/src/config/compression.rs`
- Modify: `crates/hermes-core/src/config/mod.rs`

- [ ] Define `CompressionConfig` struct
- [ ] Add compression settings to Config
- [ ] Implement compression validation
- [ ] Add compression tests

### Phase 4 (P2 - Medium): Auxiliary Models

**Files:**
- Create: `crates/hermes-core/src/config/auxiliary.rs`
- Modify: `crates/hermes-core/src/config/mod.rs`

- [ ] Define `AuxiliaryConfig`, `VisionConfig`, `WebExtractConfig`
- [ ] Add auxiliary settings to Config
- [ ] Implement validation
- [ ] Add auxiliary tests

### Phase 5 (P2 - Medium): MCP Servers & STT

**Files:**
- Create: `crates/hermes-core/src/config/mcp.rs`
- Create: `crates/hermes-core/src/config/stt.rs`
- Modify: `crates/hermes-core/src/config/mod.rs`

- [ ] Define `McpServersConfig` with Stdio/Http transport
- [ ] Define `SttConfig` with multi-provider support
- [ ] Add to Config and implement validation
- [ ] Add tests

### Phase 6 (P2 - Medium): Delegation & Display

**Files:**
- Create: `crates/hermes-core/src/config/delegation.rs`
- Create: `crates/hermes-core/src/config/display.rs`
- Modify: `crates/hermes-core/src/config/mod.rs`

- [ ] Define `DelegationConfig` with subagent settings
- [ ] Define `DisplayConfig` with UI settings
- [ ] Add to Config and implement validation
- [ ] Add tests

### Phase 7 (P3 - Lower): Personality Config

**Files:**
- Create: `crates/hermes-core/src/config/personality.rs`
- Modify: `crates/hermes-core/src/config/mod.rs`
- Modify: `crates/hermes-cli/src/configfiles.rs`

- [ ] Define `PersonalityConfig` with preset list
- [ ] Define `PersonalityPreset` with 14 presets
- [ ] Add personality loading from config
- [ ] Implement CLI personality flag
- [ ] Add tests

---

## File Map

```
crates/hermes-core/src/config/
├── mod.rs                    # Config struct, re-exports
├── provider.rs               # ProviderSettings, SmartRouterConfig
├── provider_core.rs          # CoreProvider enum
├── provider_custom.rs        # Custom provider variant
├── backend.rs                # BackendSettings, BackendConfig
├── backend_local.rs          # LocalBackend
├── backend_docker.rs         # DockerBackend
├── backend_ssh.rs            # SSHBackend
├── backend_singularity.rs    # SingularityBackend
├── backend_modal.rs          # ModalBackend
├── backend_daytona.rs        # DaytonaBackend
├── compression.rs            # CompressionConfig
├── auxiliary.rs             # AuxiliaryConfig
├── mcp.rs                    # McpServersConfig
├── stt.rs                    # SttConfig
├── delegation.rs             # DelegationConfig
├── display.rs                # DisplayConfig
└── personality.rs            # PersonalityConfig
```

---

## Validation Rules

1. **Provider priority chain must be valid**: Each provider name must exist in providers list
2. **Smart router cheap_threshold**: Must be between 0.0 and 1.0
3. **Backend must be available**: For selected backend type, required fields must be present
4. **Compression threshold**: Must be > 0
5. **MCP server transport**: Stdio requires command, Http requires valid URL
6. **STT provider name uniqueness**: No duplicate provider names
7. **Personality default name**: Must exist in personalities list if specified

---

## Testing Strategy

- Unit tests for each config struct (validation, defaults, serialization)
- Integration tests for Settings -> Config conversion
- Test invalid configs fail with appropriate errors
- Test all 7 backend types serialize/deserialize correctly
- Test personality presets load from TOML
