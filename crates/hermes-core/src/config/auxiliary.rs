use serde::{Deserialize, Serialize};

/// Vision model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionConfig {
    #[serde(default = "default_vision_provider")]
    pub provider: String,
    #[serde(default = "default_vision_model")]
    pub model: String,
}

fn default_vision_provider() -> String {
    "openai".to_string()
}
fn default_vision_model() -> String {
    "openai/gpt-4o".to_string()
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "openai/gpt-4o".to_string(),
        }
    }
}

/// Web extraction model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebExtractConfig {
    #[serde(default = "default_web_provider")]
    pub provider: String,
    #[serde(default = "default_web_model")]
    pub model: String,
}

fn default_web_provider() -> String {
    "openai".to_string()
}
fn default_web_model() -> String {
    "openai/gpt-4o-mini".to_string()
}

impl Default for WebExtractConfig {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "openai/gpt-4o-mini".to_string(),
        }
    }
}

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
