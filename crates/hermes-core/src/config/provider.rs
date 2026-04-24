use serde::{Deserialize, Serialize};

/// Per-model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelConfig {
    pub name: String,
    pub provider: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_cache_priority")]
    pub cache_priority: i32,
    #[serde(default)]
    pub context_window: Option<u32>,
    #[serde(default = "default_supports_function_calls")]
    pub supports_function_calls: bool,
}

fn default_enabled() -> bool {
    true
}
fn default_cache_priority() -> i32 {
    0
}
fn default_supports_function_calls() -> bool {
    false
}

/// Smart router configuration for automatic model selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRouterConfig {
    #[serde(default = "default_router_enabled")]
    pub enabled: bool,
    pub cheap_model: String,
    #[serde(default = "default_cheap_threshold")]
    pub cheap_threshold: f32,
    #[serde(default = "default_cheap_max_tokens")]
    pub cheap_max_tokens: u32,
    pub default_model: String,
}

fn default_router_enabled() -> bool {
    false
}
fn default_cheap_threshold() -> f32 {
    0.3
}
fn default_cheap_max_tokens() -> u32 {
    4096
}

/// Provider settings with routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub default: String,
    #[serde(default)]
    pub priority: Vec<String>,
    pub fallback: String,
    #[serde(default)]
    pub smart_router: SmartRouterConfig,
    pub allowed_rrt: Option<u32>,
    #[serde(default)]
    pub models: Vec<ProviderModelConfig>,
}

impl Default for SmartRouterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cheap_model: String::new(),
            cheap_threshold: 0.3,
            cheap_max_tokens: 4096,
            default_model: String::new(),
        }
    }
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            default: "openai/gpt-4o".to_string(),
            priority: Vec::new(),
            fallback: "openai/gpt-4o".to_string(),
            smart_router: SmartRouterConfig::default(),
            allowed_rrt: None,
            models: Vec::new(),
        }
    }
}
