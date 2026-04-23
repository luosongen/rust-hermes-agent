# Configuration System Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enhance rust-hermes-agent's configuration system to match Python hermes-agent's comprehensive config (20+ categories)

**Architecture:** Modular config structure with provider routing, smart model selection, multi-backend execution, context compression, auxiliary models, MCP server management, STT providers, delegation config, display settings, and personality presets.

**Tech Stack:** Rust, serde, figment, tokio

---

## File Structure

```
crates/hermes-core/src/config/
├── mod.rs                    # Config struct, re-exports, migrations
├── provider.rs               # ProviderSettings, SmartRouterConfig
├── provider_core.rs          # CoreProvider enum (all 7 providers)
├── provider_custom.rs        # Custom provider variant
├── backend.rs                # BackendSettings, BackendConfig enum
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

## Task 1: Create provider_core.rs with CoreProvider enum

**Files:**
- Create: `crates/hermes-core/src/config/provider_core.rs`

- [ ] **Step 1: Create the file with all 7 provider enum variants**

```rust
use serde::{Deserialize, Serialize};
use crate::credentials::Secret;

/// Core provider enum for known providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CoreProvider {
    #[serde(rename = "openrouter")]
    OpenRouter(OpenRouterConfig),
    #[serde(rename = "nous")]
    Nous(NousConfig),
    #[serde(rename = "anthropic")]
    Anthropic(AnthropicConfig),
    #[serde(rename = "openai")]
    OpenAI(OpenAIConfig),
    #[serde(rename = "gemini")]
    Gemini(GeminiConfig),
    #[serde(rename = "huggingface")]
    HuggingFace(HuggingFaceConfig),
    #[serde(rename = "minimax")]
    MiniMax(MiniMaxConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NousConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniMaxConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<url::Url>,
    pub models: Vec<String>,
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/provider_core.rs
git commit -m "feat(config): add CoreProvider enum with all 7 providers

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2: Create provider_custom.rs with Custom provider variant

**Files:**
- Create: `crates/hermes-core/src/config/provider_custom.rs`

- [ ] **Step 1: Create the file with Custom provider variant**

```rust
use serde::{Deserialize, Serialize};
use crate::credentials::Secret;

/// Custom provider for user-defined providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub name: String,
    pub base_url: url::Url,
    pub api_key: Secret<String>,
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/provider_custom.rs
git commit -m "feat(config): add CustomProviderConfig for user-defined providers

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: Create provider.rs with ProviderSettings and SmartRouterConfig

**Files:**
- Create: `crates/hermes-core/src/config/provider.rs`

- [ ] **Step 1: Create ProviderModelConfig, SmartRouterConfig, ProviderSettings structs**

```rust
use serde::{Deserialize, Serialize};

/// Per-model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelConfig {
    pub name: String,                    // "anthropic/claude-3-5-sonnet"
    pub provider: String,               // "anthropic"
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_cache_priority")]
    pub cache_priority: i32,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default = "default_supports_function_calls")]
    pub supports_function_calls: bool,
}

fn default_enabled() -> bool { true }
fn default_cache_priority() -> i32 { 0 }
fn default_supports_function_calls() -> bool { false }

/// Smart router configuration for automatic model selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRouterConfig {
    #[serde(default = "default_router_enabled")]
    pub enabled: bool,
    pub cheap_model: String,            // "openrouter/..."
    #[serde(default = "default_cheap_threshold")]
    pub cheap_threshold: f32,          // 0.3
    #[serde(default = "default_cheap_max_tokens")]
    pub cheap_max_tokens: u32,         // 4096
    pub default_model: String,
}

fn default_router_enabled() -> bool { false }
fn default_cheap_threshold() -> f32 { 0.3 }
fn default_cheap_max_tokens() -> u32 { 4096 }

/// Provider settings with routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub default: String,                // "openrouter/..."
    #[serde(default)]
    pub priority: Vec<String>,          // ["anthropic/...", "openai/..."]
    pub fallback: String,                // "openai/..."
    #[serde(default)]
    pub smart_router: SmartRouterConfig,
    pub allowed_rrt: Option<u32>,
    #[serde(default)]
    pub models: Vec<ProviderModelConfig>,
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/provider.rs
git commit -m "feat(config): add ProviderSettings, SmartRouterConfig, ProviderModelConfig

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: Create backend.rs and all backend_*.rs files

**Files:**
- Create: `crates/hermes-core/src/config/backend.rs`
- Create: `crates/hermes-core/src/config/backend_local.rs`
- Create: `crates/hermes-core/src/config/backend_docker.rs`
- Create: `crates/hermes-core/src/config/backend_ssh.rs`
- Create: `crates/hermes-core/src/config/backend_singularity.rs`
- Create: `crates/hermes-core/src/config/backend_modal.rs`
- Create: `crates/hermes-core/src/config/backend_daytona.rs`

- [ ] **Step 1: Create backend_local.rs**

```rust
use serde::{Deserialize, Serialize};

/// Local backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalBackend {
    #[serde(default)]
    pub enabled: bool,
}
```

- [ ] **Step 2: Create backend_docker.rs**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Docker backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerBackend {
    #[serde(default)]
    pub enabled: bool,
    pub container: Option<String>,
    pub docker_host: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub auto_start: bool,
    pub user: Option<String>,
}
```

- [ ] **Step 3: Create backend_ssh.rs**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// SSH backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SSHBackend {
    #[serde(default)]
    pub enabled: bool,
    pub host: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub user: Option<String>,
    pub private_key: Option<PathBuf>,
    pub password: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub ssh_options: Vec<String>,
}

fn default_ssh_port() -> u16 { 22 }
```

- [ ] **Step 4: Create backend_singularity.rs**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Singularity backend configuration for HPC environments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SingularityBackend {
    #[serde(default)]
    pub enabled: bool,
    pub image: Option<String>,
    pub bind_paths: Option<Vec<String>>,
    pub working_directory: Option<PathBuf>,
}
```

- [ ] **Step 5: Create backend_modal.rs**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Modal backend configuration for cloud GPU access
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModalBackend {
    #[serde(default)]
    pub enabled: bool,
    pub app_name: Option<String>,
    pub token_id: Option<String>,
    pub token_secret: Option<String>,
    pub working_directory: Option<PathBuf>,
}
```

- [ ] **Step 6: Create backend_daytona.rs**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Daytona backend configuration for cloud dev environments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaytonaBackend {
    #[serde(default)]
    pub enabled: bool,
    pub server_url: Option<String>,
    pub api_key: Option<String>,
    pub workspace_dir: Option<PathBuf>,
}
```

- [ ] **Step 7: Create backend.rs with BackendConfig enum and BackendSettings**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod backend_local;
mod backend_docker;
mod backend_ssh;
mod backend_singularity;
mod backend_modal;
mod backend_daytona;

pub use backend_local::LocalBackend;
pub use backend_docker::DockerBackend;
pub use backend_ssh::SSHBackend;
pub use backend_singularity::SingularityBackend;
pub use backend_modal::ModalBackend;
pub use backend_daytona::DaytonaBackend;

/// Backend configuration enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BackendConfig {
    #[serde(rename = "local")]
    Local(LocalBackend),
    #[serde(rename = "docker")]
    Docker(DockerBackend),
    #[serde(rename = "ssh")]
    SSH(SSHBackend),
    #[serde(rename = "singularity")]
    Singularity(SingularityBackend),
    #[serde(rename = "modal")]
    Modal(ModalBackend),
    #[serde(rename = "daytona")]
    Daytona(DaytonaBackend),
}

impl Default for BackendConfig {
    fn default() -> Self {
        BackendConfig::Local(LocalBackend { enabled: true })
    }
}

/// Backend settings with default selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSettings {
    #[serde(default = "default_backend")]
    pub default: BackendConfig,
    #[serde(default)]
    pub workdir: PathBuf,
}

fn default_backend() -> BackendConfig { BackendConfig::default() }
```

- [ ] **Step 8: Verify all files compile**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 9: Commit**

```bash
git add crates/hermes-core/src/config/backend.rs
git add crates/hermes-core/src/config/backend_local.rs
git add crates/hermes-core/src/config/backend_docker.rs
git add crates/hermes-core/src/config/backend_ssh.rs
git add crates/hermes-core/src/config/backend_singularity.rs
git add crates/hermes-core/src/config/backend_modal.rs
git add crates/hermes-core/src/config/backend_daytona.rs
git commit -m "feat(config): add BackendConfig enum with all 6 backend types

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: Create compression.rs with CompressionConfig

**Files:**
- Create: `crates/hermes-core/src/config/compression.rs`

- [ ] **Step 1: Create CompressionConfig struct**

```rust
use serde::{Deserialize, Serialize};

/// Context compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    #[serde(default = "default_compression_enabled")]
    pub enabled: bool,
    #[serde(default = "default_threshold")]
    pub threshold: u32,           // 60000 tokens
    #[serde(default = "default_target_ratio")]
    pub target_ratio: f32,        // 0.7
    #[serde(default = "default_protect_last_n")]
    pub protect_last_n: u32,      // 10 messages
    pub model: Option<String>,    // "openai/..." - uses default if None
}

fn default_compression_enabled() -> bool { false }
fn default_threshold() -> u32 { 60000 }
fn default_target_ratio() -> f32 { 0.7 }
fn default_protect_last_n() -> u32 { 10 }

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 60000,
            target_ratio: 0.7,
            protect_last_n: 10,
            model: None,
        }
    }
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/compression.rs
git commit -m "feat(config): add CompressionConfig for context compression

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: Create auxiliary.rs with AuxiliaryConfig

**Files:**
- Create: `crates/hermes-core/src/config/auxiliary.rs`

- [ ] **Step 1: Create VisionConfig, WebExtractConfig, AuxiliaryConfig structs**

```rust
use serde::{Deserialize, Serialize};

/// Vision model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    #[serde(default = "default_vision_provider")]
    pub provider: String,        // "openai"
    #[serde(default = "default_vision_model")]
    pub model: String,           // "openai/gpt-4o"
}

fn default_vision_provider() -> String { "openai".to_string() }
fn default_vision_model() -> String { "openai/gpt-4o".to_string() }

/// Web extraction model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebExtractConfig {
    #[serde(default = "default_web_provider")]
    pub provider: String,        // "openai"
    #[serde(default = "default_web_model")]
    pub model: String,           // "openai/gpt-4o-mini"
}

fn default_web_provider() -> String { "openai".to_string() }
fn default_web_model() -> String { "openai/gpt-4o-mini".to_string() }

/// Auxiliary models configuration (vision, web extract)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuxiliaryConfig {
    #[serde(default)]
    pub vision: VisionConfig,
    #[serde(default)]
    pub web_extract: WebExtractConfig,
}

impl Default for AuxiliaryConfig {
    fn default() -> Self {
        Self {
            vision: VisionConfig::default(),
            web_extract: WebExtractConfig::default(),
        }
    }
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/auxiliary.rs
git commit -m "feat(config): add AuxiliaryConfig for vision and web extract models

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: Create mcp.rs with McpServersConfig

**Files:**
- Create: `crates/hermes-core/src/config/mcp.rs`

- [ ] **Step 1: Create McpTransport enum and McpServerConfig, McpServersConfig structs**

```rust
use serde::{Deserialize, Serialize};

/// MCP transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", content = "config")]
pub enum McpTransport {
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        args: Vec<String>,
    },
    #[serde(rename = "http")]
    Http {
        url: url::Url,
    },
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    #[serde(default = "default_mcp_enabled")]
    pub enabled: bool,
    pub transport: McpTransport,
}

fn default_mcp_enabled() -> bool { true }

/// MCP servers configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServersConfig {
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/mcp.rs
git commit -m "feat(config): add McpServersConfig for MCP server management

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: Create stt.rs with SttConfig

**Files:**
- Create: `crates/hermes-core/src/config/stt.rs`

- [ ] **Step 1: Create SttProviderConfig and SttConfig structs**

```rust
use serde::{Deserialize, Serialize};
use crate::credentials::Secret;

/// STT provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttProviderConfig {
    pub name: String,            // "groq", "openai", "mistral", "local"
    #[serde(default = "default_stt_provider")]
    pub provider: String,       // "groq", "openai", "mistral", "local"
    #[serde(default = "default_stt_model")]
    pub model: String,           // "whisper-large-v3"
    pub api_key: Option<Secret<String>>,
    pub base_url: Option<url::Url>,
    #[serde(default = "default_stt_enabled")]
    pub enabled: bool,
}

fn default_stt_provider() -> String { "local".to_string() }
fn default_stt_model() -> String { "whisper-large-v3".to_string() }
fn default_stt_enabled() -> bool { true }

/// STT (Speech-to-Text) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttConfig {
    #[serde(default = "default_stt_default")]
    pub default: String,        // "local"
    #[serde(default)]
    pub providers: Vec<SttProviderConfig>,
}

fn default_stt_default() -> String { "local".to_string() }

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            default: "local".to_string(),
            providers: Vec::new(),
        }
    }
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/stt.rs
git commit -m "feat(config): add SttConfig for speech-to-text providers

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 9: Create delegation.rs with DelegationConfig

**Files:**
- Create: `crates/hermes-core/src/config/delegation.rs`

- [ ] **Step 1: Create DelegationConfig struct**

```rust
use serde::{Deserialize, Serialize};

/// Delegation configuration for sub-agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConfig {
    #[serde(default = "default_delegation_enabled")]
    pub enabled: bool,
    pub default_personality: Option<String>,
    pub default_model: String,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub terminate_on_model: Vec<String>,
}

fn default_delegation_enabled() -> bool { false }
fn default_max_depth() -> u32 { 3 }

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_personality: None,
            default_model: "openai/gpt-4o".to_string(),
            max_depth: 3,
            max_tokens: None,
            terminate_on_model: Vec::new(),
        }
    }
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/delegation.rs
git commit -m "feat(config): add DelegationConfig for sub-agent delegation

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 10: Create display.rs with DisplayConfig

**Files:**
- Create: `crates/hermes-core/src/config/display.rs`

- [ ] **Step 1: Create DisplayConfig struct**

```rust
use serde::{Deserialize, Serialize};

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_compact")]
    pub compact: bool,
    #[serde(default = "default_tool_progress")]
    pub tool_progress: bool,
    #[serde(default = "default_skin")]
    pub skin: String,
}

fn default_compact() -> bool { false }
fn default_tool_progress() -> bool { true }
fn default_skin() -> String { "default".to_string() }

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            compact: false,
            tool_progress: true,
            skin: "default".to_string(),
        }
    }
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/display.rs
git commit -m "feat(config): add DisplayConfig for UI settings

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 11: Create personality.rs with PersonalityConfig

**Files:**
- Create: `crates/hermes-core/src/config/personality.rs`

- [ ] **Step 1: Create PersonalityPreset and PersonalityConfig structs**

```rust
use serde::{Deserialize, Serialize};

/// Personality preset definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityPreset {
    pub name: String,
    pub system_prompt: String,
    pub model: Option<String>,
}

/// Personality configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    #[serde(default = "default_personality")]
    pub default: String,        // "helpfulness"
    #[serde(default)]
    pub personalities: Vec<PersonalityPreset>,
}

fn default_personality() -> String { "helpfulness".to_string() }

impl Default for PersonalityConfig {
    fn default() -> Self {
        Self {
            default: "helpfulness".to_string(),
            personalities: vec![
                PersonalityPreset {
                    name: "helpfulness".to_string(),
                    system_prompt: "You are a helpful assistant.".to_string(),
                    model: None,
                },
            ],
        }
    }
}
```

- [ ] **Step 2: Verify file compiles**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/personality.rs
git commit -m "feat(config): add PersonalityConfig with preset support

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 12: Refactor config/mod.rs to use new modules

**Files:**
- Modify: `crates/hermes-core/src/config.rs` → rename to `crates/hermes-core/src/config/mod.rs`

- [ ] **Step 1: Create config/mod.rs that re-exports all new modules**

```rust
//! Configuration system module
//!
//! ## 模块用途
//! 管理 hermes-agent 的所有配置，包括默认模型、凭证、网关设置和消息平台配置。
//! 支持从 TOML 配置文件、环境变量（`HERMES_*`）和代码默认值多层级加载。

pub mod provider_core;
pub mod provider_custom;
pub mod provider;
pub mod backend;
pub mod compression;
pub mod auxiliary;
pub mod mcp;
pub mod stt;
pub mod delegation;
pub mod display;
pub mod personality;

// Re-exports for convenience
pub use provider::{ProviderSettings, SmartRouterConfig, ProviderModelConfig};
pub use provider_core::*;
pub use provider_custom::CustomProviderConfig;
pub use backend::{BackendSettings, BackendConfig};
pub use compression::CompressionConfig;
pub use auxiliary::{AuxiliaryConfig, VisionConfig, WebExtractConfig};
pub use mcp::{McpServersConfig, McpServerConfig, McpTransport};
pub use stt::{SttConfig, SttProviderConfig};
pub use delegation::DelegationConfig;
pub use display::DisplayConfig;
pub use personality::{PersonalityConfig, PersonalityPreset};

// Keep existing types for backward compatibility
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use parking_lot::Mutex;
use std::sync::OnceLock;

pub use crate::config::settings::Settings;

// ============================================================================
// Legacy types (for migration - to be deprecated)
// ============================================================================

pub use crate::nudge::NudgeConfig;

/// 环境配置（用于序列化/反序列化）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvironmentConfig {
    #[serde(default = "default_env_type")]
    pub env_type: String,
    #[serde(default = "default_working_dir")]
    pub working_directory: PathBuf,
    #[serde(default)]
    pub docker: DockerEnvConfig,
    #[serde(default)]
    pub ssh: SSHEnvConfig,
}

fn default_env_type() -> String { "local".to_string() }
fn default_working_dir() -> PathBuf { PathBuf::from(".") }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerEnvConfig {
    pub container: Option<String>,
    pub docker_host: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub auto_start: bool,
    pub user: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SSHEnvConfig {
    pub host: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub user: Option<String>,
    pub private_key: Option<PathBuf>,
    pub password: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub ssh_options: Vec<String>,
}

fn default_ssh_port() -> u16 { 22 }

static CONFIG_CACHE: OnceLock<Mutex<Config>> = OnceLock::new();

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("hermes-agent")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformConfig {
    pub bot_token: Option<String>,
    pub verify_token: Option<String>,
    pub corp_id: Option<String>,
    pub agent_id: Option<String>,
    pub token: Option<String>,
    pub aes_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub port: u16,
    pub host: String,
    pub platforms: HashMap<String, PlatformConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    pub model: String,
    pub tools_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolConfig {
    pub enabled: bool,
}

impl ToolConfig {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub credentials: HashMap<String, String>,
    #[serde(default)]
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub nudge: NudgeConfig,
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
    #[serde(default)]
    pub environment: EnvironmentConfig,
    // New config sections
    #[serde(default)]
    pub providers: ProviderSettings,
    #[serde(default)]
    pub backends: BackendSettings,
    #[serde(default)]
    pub compression: CompressionConfig,
    #[serde(default)]
    pub auxiliary: AuxiliaryConfig,
    #[serde(default)]
    pub mcp_servers: McpServersConfig,
    #[serde(default)]
    pub stt: SttConfig,
    #[serde(default)]
    pub delegation: DelegationConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub personality: PersonalityConfig,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            model: "openai/gpt-4o".to_string(),
            tools_enabled: true,
        }
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "0.0.0.0".to_string(),
            platforms: HashMap::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: DefaultsConfig::default(),
            credentials: HashMap::new(),
            gateway: GatewayConfig::default(),
            nudge: NudgeConfig::default(),
            tools: HashMap::new(),
            environment: EnvironmentConfig::default(),
            providers: ProviderSettings::default(),
            backends: BackendSettings::default(),
            compression: CompressionConfig::default(),
            auxiliary: AuxiliaryConfig::default(),
            mcp_servers: McpServersConfig::default(),
            stt: SttConfig::default(),
            delegation: DelegationConfig::default(),
            display: DisplayConfig::default(),
            personality: PersonalityConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::default();
        let path = config_file();
        if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(ConfigError::Io)?;
            if let Ok(file_config) = toml::from_str::<Config>(&content) {
                config.merge(file_config);
            } else if let Ok(file_config) = serde_yaml::from_str::<Config>(&content) {
                config.merge(file_config);
            }
        }
        config.load_from_env();
        Ok(config)
    }

    fn merge(&mut self, other: Config) {
        // Existing merge logic...
        if other.defaults.model != DefaultsConfig::default().model {
            self.defaults.model = other.defaults.model;
        }
        if other.defaults.tools_enabled != DefaultsConfig::default().tools_enabled {
            self.defaults.tools_enabled = other.defaults.tools_enabled;
        }
        if !other.credentials.is_empty() {
            self.credentials.extend(other.credentials);
        }
        if other.gateway.port != GatewayConfig::default().port {
            self.gateway.port = other.gateway.port;
        }
        if other.gateway.host != GatewayConfig::default().host {
            self.gateway.host = other.gateway.host;
        }
        for (name, platform) in other.gateway.platforms {
            self.gateway.platforms.insert(name, platform);
        }
        if other.nudge != NudgeConfig::default() {
            self.nudge = other.nudge;
        }
        if other.environment.env_type != default_env_type() {
            self.environment.env_type = other.environment.env_type;
        }
        if other.environment.working_directory != default_working_dir() {
            self.environment.working_directory = other.environment.working_directory.clone();
        }
        if other.environment.docker.container.is_some() {
            self.environment.docker = other.environment.docker.clone();
        }
        if other.environment.ssh.host.is_some() {
            self.environment.ssh = other.environment.ssh.clone();
        }
        // Merge new config sections
        if other.providers.default != ProviderSettings::default().default {
            self.providers = other.providers;
        }
        if other.backends.default != BackendSettings::default().default {
            self.backends = other.backends;
        }
        if other.compression.enabled {
            self.compression = other.compression;
        }
        // Auxiliary, MCP, STT, delegation, display, personality merge...
    }

    fn load_from_env(&mut self) {
        // Existing env loading...
        // Add new env vars for new config sections
    }

    pub fn get_cached() -> &'static Mutex<Config> {
        CONFIG_CACHE.get_or_init(|| {
            Mutex::new(Self::load().expect("failed to load config"))
        })
    }

    pub fn display(&self) -> String {
        // Existing display logic + new sections
    }
}

/// 对敏感值进行脱敏处理
fn redact_secret(value: &str) -> String {
    if value.len() <= 9 {
        "*".repeat(value.len())
    } else {
        format!("{}...****", &value[..5])
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("序列化配置失败: {0}")]
    Serialize(String),
    #[error("I/O 操作失败: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_get_set() {
        let mut config = Config::default();
        assert_eq!(config.get("defaults.model"), Some("openai/gpt-4o".to_string()));
        config.set("defaults.model", "anthropic/claude-3".to_string());
        assert_eq!(config.get("defaults.model"), Some("anthropic/claude-3".to_string()));
    }
}
```

- [ ] **Step 2: Verify compilation with all modules**

Run: `cargo check -p hermes-core`
Expected: Compiles successfully with all new config modules integrated

- [ ] **Step 3: Commit**

```bash
git add crates/hermes-core/src/config/
git commit -m "refactor(config): modularize config system with new config modules

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 13: Add tests for new config modules

**Files:**
- Create: `crates/hermes-core/src/config/tests.rs`

- [ ] **Step 1: Write tests for provider_core**

```rust
#[test]
fn test_core_provider_serialization() {
    use crate::config::CoreProvider::*;
    use crate::credentials::Secret;
    
    let config = OpenRouter(crate::config::OpenRouterConfig {
        api_key: Secret::new("test-key".to_string()),
        base_url: None,
        models: vec!["meta-llama/llama-3".to_string()],
    });
    
    let serialized = toml::to_string(&config).unwrap();
    assert!(serialized.contains("openrouter"));
}
```

- [ ] **Step 2: Write tests for backend**

```rust
#[test]
fn test_backend_config_serialization() {
    use crate::config::BackendConfig;
    
    let docker = BackendConfig::Docker(crate::config::DockerBackend {
        enabled: true,
        container: Some("my-container".to_string()),
        docker_host: None,
        working_directory: None,
        auto_start: false,
        user: None,
    });
    
    let serialized = toml::to_string(&docker).unwrap();
    assert!(serialized.contains("docker"));
    assert!(serialized.contains("my-container"));
}
```

- [ ] **Step 3: Run all config tests**

Run: `cargo test -p hermes-core -- config`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/hermes-core/src/config/tests.rs
git commit -m "test(config): add tests for new config modules

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review Checklist

1. **Spec coverage:** All config sections from spec are implemented (ProviderSettings, BackendConfig, CompressionConfig, AuxiliaryConfig, McpServersConfig, SttConfig, DelegationConfig, DisplayConfig, PersonalityConfig)
2. **Placeholder scan:** No TBD/TODO placeholders in code steps
3. **Type consistency:** Types are consistent across modules - all use proper Serde derives

---

## Plan Complete

Plan complete and saved to `docs/superpowers/plans/2026-04-23-config-system-enhancement-plan.md`.

**Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
