use serde::{Deserialize, Serialize};

/// Context compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    #[serde(default = "default_compression_enabled")]
    pub enabled: bool,
    #[serde(default = "default_threshold")]
    pub threshold: u32,
    #[serde(default = "default_target_ratio")]
    pub target_ratio: f32,
    #[serde(default = "default_protect_last_n")]
    pub protect_last_n: u32,
    pub model: Option<String>,
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
