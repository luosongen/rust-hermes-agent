use crate::credential_pool::Secret;
use serde::{Deserialize, Serialize};

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
    pub base_url: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NousConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniMaxConfig {
    pub api_key: Secret<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
}
