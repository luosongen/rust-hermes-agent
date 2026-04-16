//! VisionTool — 图像分析工具
//!
//! 调用云服务视觉模型（GPT-4V / Claude Vision / Gemini）分析图像内容。

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hermes_core::{Content, LlmProvider, Message, ModelId, ChatRequest, ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde_json::json;
use std::sync::Arc;

// ============================================================================
// VisionProvider enum — all supported vision backends
// ============================================================================

#[derive(Clone)]
pub enum VisionProvider {
    Llm(LlmVisionProvider),
    Anthropic(AnthropicVisionProvider),
    OpenAi(OpenAIVisionProvider),
}

impl VisionProvider {
    pub async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError> {
        match self {
            VisionProvider::Llm(p) => p.analyze(image, question).await,
            VisionProvider::Anthropic(p) => p.analyze(image, question).await,
            VisionProvider::OpenAi(p) => p.analyze(image, question).await,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            VisionProvider::Llm(_) => "llm",
            VisionProvider::Anthropic(_) => "anthropic",
            VisionProvider::OpenAi(_) => "openai",
        }
    }
}

#[derive(Clone, Debug)]
pub enum VisionImage {
    Url(String),
    Base64(String),
}

#[derive(Clone, Debug)]
pub struct VisionResult {
    pub analysis: String,
    pub model: String,
}

// ============================================================================
// Anthropic Claude Vision provider
// ============================================================================

#[derive(Clone)]
pub struct AnthropicVisionProvider {
    api_key: String,
    model: String,
    http_client: reqwest::Client,
}

impl AnthropicVisionProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-3-5-sonnet-20241022".to_string(),
            http_client: reqwest::Client::new(),
        }
    }
}

impl AnthropicVisionProvider {
    async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError> {
        let (media_type, data) = match &image {
            VisionImage::Url(url) => {
                let resp = self.http_client.get(url).send().await
                    .map_err(|e| ToolError::Execution(format!("Failed to download image: {}", e)))?;
                let bytes = resp.bytes().await
                    .map_err(|e| ToolError::Execution(format!("Failed to read image bytes: {}", e)))?;
                let base64_str = BASE64.encode(&bytes);
                ("image/jpeg".to_string(), base64_str)
            }
            VisionImage::Base64(b64) => {
                ("image/jpeg".to_string(), b64.clone())
            }
        };

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": media_type,
                            "data": data
                        }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }]
        });

        let resp = self.http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Anthropic API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Anthropic response: {}", e)))?;

        if let Some(error) = body.get("error") {
            return Err(ToolError::Execution(
                error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error").to_string()
            ));
        }

        let content = body["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("No analysis returned")
            .to_string();

        Ok(VisionResult {
            analysis: content,
            model: self.model.clone(),
        })
    }
}

// ============================================================================
// OpenAI Vision Provider (GPT-4V)
// ============================================================================

#[derive(Clone)]
pub struct OpenAIVisionProvider {
    api_key: String,
    model: String,
    http_client: reqwest::Client,
}

impl OpenAIVisionProvider {
    pub fn new(api_key: String, model: &str) -> Self {
        Self {
            api_key,
            model: model.to_string(),
            http_client: reqwest::Client::new(),
        }
    }
}

impl OpenAIVisionProvider {
    async fn analyze(&self, image: VisionImage, question: &str) -> Result<VisionResult, ToolError> {
        let (url, detail) = match &image {
            VisionImage::Url(u) => (u.clone(), "auto".to_string()),
            VisionImage::Base64(b64) => (format!("data:image/jpeg;base64,{}", b64), "auto".to_string()),
        };

        let payload = serde_json::json!({
            "model": self.model,
            "messages": [{
                "role": "user",
                "content": [
                    {
                        "type": "image_url",
                        "image_url": { "url": url, "detail": detail }
                    },
                    {
                        "type": "text",
                        "text": question
                    }
                ]
            }]
        });

        let resp = self.http_client
            .post("https://api.openai.com/v1/chat/completions")
            .header("authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenAI API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid OpenAI response: {}", e)))?;

        let content = body["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("No analysis returned")
            .to_string();

        Ok(VisionResult {
            analysis: content,
            model: self.model.clone(),
        })
    }
}

// ============================================================================
// LlmVisionProvider — wraps LlmProvider for backward compatibility
// ============================================================================

#[derive(Clone)]
pub struct LlmVisionProvider {
    provider: Arc<dyn LlmProvider>,
}

impl LlmVisionProvider {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }
}

impl LlmVisionProvider {
    async fn analyze(&self, image: VisionImage, _question: &str) -> Result<VisionResult, ToolError> {
        let content = match &image {
            VisionImage::Url(url) => Content::Image { url: url.clone(), detail: None },
            VisionImage::Base64(b64) => {
                Content::Image {
                    url: format!("data:image/jpeg;base64,{}", b64),
                    detail: None,
                }
            }
        };

        let message = Message::user(content);
        let request = ChatRequest {
            model: ModelId::new("openai", "gpt-4o"),
            messages: vec![message],
            tools: None,
            system_prompt: None,
            temperature: None,
            max_tokens: None,
        };

        let response = self.provider.chat(request).await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        if response.content.is_empty() {
            return Err(ToolError::Execution("Empty response from vision model".to_string()));
        }

        Ok(VisionResult {
            analysis: response.content,
            model: "gpt-4o".to_string(),
        })
    }
}

// ============================================================================
// VisionTool
// ============================================================================

/// VisionTool — 图像分析工具，支持多 provider
#[derive(Clone)]
pub struct VisionTool {
    providers: std::collections::HashMap<String, VisionProvider>,
}

impl std::fmt::Debug for VisionTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisionTool").finish()
    }
}

impl VisionTool {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        let mut providers = std::collections::HashMap::new();
        providers.insert("llm".to_string(), VisionProvider::Llm(LlmVisionProvider::new(provider)));
        Self { providers }
    }

    pub fn with_openai(mut self, api_key: String, model: &str) -> Self {
        self.providers.insert(
            "openai".to_string(),
            VisionProvider::OpenAi(OpenAIVisionProvider::new(api_key, model))
        );
        self
    }

    pub fn with_anthropic(mut self, api_key: String) -> Self {
        self.providers.insert(
            "anthropic".to_string(),
            VisionProvider::Anthropic(AnthropicVisionProvider::new(api_key))
        );
        self
    }

    fn is_likely_base64(input: &str) -> bool {
        // Base64 strings only contain A-Z, a-z, 0-9, +, /, and = for padding
        input.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
            && !input.contains('/')
            && !input.contains('\\')
            && !input.contains(':')
            && input.len() >= 4
    }

    pub fn parse_image(input: &str) -> VisionImage {
        if input.starts_with("data:") || input.starts_with("http://") || input.starts_with("https://") {
            VisionImage::Url(input.to_string())
        } else if Self::is_likely_base64(input) {
            VisionImage::Base64(input.to_string())
        } else {
            let data = std::fs::read(input).unwrap_or_default();
            let base64_str = BASE64.encode(&data);
            VisionImage::Base64(base64_str)
        }
    }
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str { "vision_analyze" }

    fn description(&self) -> &str {
        "Analyze images using vision AI. Supports image URLs, local file paths, or base64-encoded images."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Image URL, local file path, or base64-encoded image"
                },
                "prompt": {
                    "type": "string",
                    "description": "Analysis question",
                    "default": "Describe this image in detail"
                },
                "provider": {
                    "type": "string",
                    "enum": ["llm", "openai", "anthropic"],
                    "default": "llm"
                }
            },
            "required": ["image"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let image_str = args["image"].as_str()
            .ok_or_else(|| ToolError::InvalidArgs("image is required".to_string()))?;
        let prompt = args["prompt"].as_str().unwrap_or("Describe this image in detail");
        let provider_name = args["provider"].as_str().unwrap_or("llm");

        let image = Self::parse_image(image_str);

        let provider = self.providers.get(provider_name)
            .ok_or_else(|| ToolError::InvalidArgs(
                format!("Unknown provider: {}. Available: {}",
                    provider_name,
                    self.providers.keys().cloned().collect::<Vec<_>>().join(", "))
            ))?;

        let result = provider.analyze(image, prompt).await?;

        Ok(serde_json::json!({
            "success": true,
            "analysis": result.analysis,
            "model": result.model
        }).to_string())
    }
}
