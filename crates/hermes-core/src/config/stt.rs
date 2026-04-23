use serde::{Deserialize, Serialize};
use crate::credentials::Secret;

/// STT provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttProviderConfig {
    pub name: String,
    #[serde(default = "default_stt_provider")]
    pub provider: String,
    #[serde(default = "default_stt_model")]
    pub model: String,
    pub api_key: Option<Secret<String>>,
    pub base_url: Option<String>,
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
    pub default: String,
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
