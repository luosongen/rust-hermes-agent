//! VisionTool — 图像分析工具
//!
//! 调用云服务视觉模型（GPT-4V / Claude Vision / Gemini）分析图像内容。

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use hermes_core::{Content, LlmProvider, Message, ModelId, ChatRequest, ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde_json::json;
use std::sync::Arc;

/// VisionTool — 图像分析工具
#[derive(Clone)]
pub struct VisionTool {
    provider: Arc<dyn LlmProvider>,
}

impl std::fmt::Debug for VisionTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisionTool").finish()
    }
}

impl VisionTool {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    fn guess_mime(path: &str) -> &'static str {
        if path.ends_with(".png") {
            "image/png"
        } else if path.ends_with(".gif") {
            "image/gif"
        } else if path.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        }
    }
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str { "vision" }

    fn description(&self) -> &str {
        "Analyze images using vision-capable LLM models. Supports image URLs and local file paths."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "image": {
                    "type": "string",
                    "description": "Image URL or local file path"
                },
                "prompt": {
                    "type": "string",
                    "description": "Analysis instruction",
                    "default": "Describe this image in detail"
                },
                "model": {
                    "type": "string",
                    "description": "Optional: vision model name override"
                }
            },
            "required": ["image"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let image = args["image"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("image is required".to_string()))?;

        let content = if image.starts_with("http://") || image.starts_with("https://") {
            Content::Image { url: image.to_string(), detail: None }
        } else {
            let data = std::fs::read(image)
                .map_err(|e| ToolError::Execution(format!("Failed to read image file: {}", e)))?;
            let base64_str = BASE64.encode(&data);
            let mime = Self::guess_mime(image);
            Content::Image {
                url: format!("data:{};base64,{}", mime, base64_str),
                detail: None,
            }
        };

        let message = Message::user(content);
        let model_str = args["model"].as_str().unwrap_or("openai/gpt-4o");
        let model_id = ModelId::parse(model_str).unwrap_or_else(|| ModelId::new("openai", "gpt-4o"));

        let request = ChatRequest {
            model: model_id,
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

        Ok(response.content)
    }
}