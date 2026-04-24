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
    pub default: String,
    #[serde(default)]
    pub personalities: Vec<PersonalityPreset>,
}

fn default_personality() -> String {
    "helpfulness".to_string()
}

impl Default for PersonalityConfig {
    fn default() -> Self {
        Self {
            default: "helpfulness".to_string(),
            personalities: vec![PersonalityPreset {
                name: "helpfulness".to_string(),
                system_prompt: "You are a helpful assistant.".to_string(),
                model: None,
            }],
        }
    }
}
