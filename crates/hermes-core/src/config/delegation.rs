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

fn default_delegation_enabled() -> bool {
    false
}
fn default_max_depth() -> u32 {
    3
}

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
