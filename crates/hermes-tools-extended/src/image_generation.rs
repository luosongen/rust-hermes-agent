//! ImageGenerationTool — 多提供商图像生成
//!
//! 支持：
//! - **fal** — Fal.ai FLUX 2 Pro（默认，带 2x 超分）
//! - **pollinations** — 免费，无需 API Key
//! - **openai** — DALL-E 3
//! - **stability** — Stability AI SD3

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const FAL_FLUX_PRO_URL: &str = "https://queue.fal.run/fal-ai/flux-2-pro";
const FAL_CLARITY_URL: &str = "https://queue.fal.run/fal-ai/clarity-upscaler";
const OPENAI_DALLE_URL: &str = "https://api.openai.com/v1/images/generations";
const STABILITY_URL: &str = "https://api.stability.ai/v2beta/stable-image/generate/sd3";
const POLLINATIONS_URL: &str = "https://image.pollinations.ai/prompt";

#[derive(Clone)]
pub struct ImageGenerationTool {
    http_client: reqwest::Client,
    fal_api_key: Option<String>,
    openai_api_key: Option<String>,
    stability_api_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ImageSize {
    #[serde(rename = "landscape_16_9")]
    Landscape16x9,
    #[serde(rename = "portrait_9_16")]
    Portrait9x16,
    #[serde(rename = "square_1_1")]
    Square1x1,
    #[serde(rename = "landscape_4_3")]
    Landscape4x3,
}

impl Default for ImageGenerationTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageGenerationTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("HTTP client"),
            fal_api_key: std::env::var("FAL_API_KEY").ok(),
            openai_api_key: std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))
                .ok(),
            stability_api_key: std::env::var("STABILITY_API_KEY").ok(),
        }
    }

    pub fn with_fal_api_key(mut self, key: String) -> Self {
        self.fal_api_key = Some(key);
        self
    }

    pub fn with_openai_key(mut self, key: String) -> Self {
        self.openai_api_key = Some(key);
        self
    }

    pub fn with_stability_key(mut self, key: String) -> Self {
        self.stability_api_key = Some(key);
        self
    }
}

// =============================================================================
// Fal.ai FLUX 2 Pro
// =============================================================================

impl ImageGenerationTool {
    async fn fal_request_image(&self, prompt: &str, size: ImageSize, num_inference_steps: u32, guidance_scale: f32) -> Result<String, ToolError> {
        let api_key = self.fal_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("FAL_API_KEY not set".to_string()))?;

        let size_str = match size {
            ImageSize::Landscape16x9 => "landscape_16_9",
            ImageSize::Portrait9x16 => "portrait_9_16",
            ImageSize::Square1x1 => "square_1_1",
            ImageSize::Landscape4x3 => "landscape_4_3",
        };

        let payload = serde_json::json!({
            "prompt": prompt,
            "image_size": size_str,
            "num_inference_steps": num_inference_steps,
            "guidance_scale": guidance_scale,
            "num_images": 1,
            "enable_safety_checker": false
        });

        let resp = self.http_client
            .post(FAL_FLUX_PRO_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Fal.ai request error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Fal.ai response error: {}", e)))?;

        let request_id = body["request_id"].as_str()
            .ok_or_else(|| ToolError::Execution("No request_id in Fal.ai response".to_string()))?;

        Ok(request_id.to_string())
    }

    async fn fal_poll_result(&self, request_id: &str) -> Result<String, ToolError> {
        let api_key = self.fal_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("FAL_API_KEY not set".to_string()))?;
        let url = format!("{}/results?request_id={}", FAL_FLUX_PRO_URL, request_id);

        for _ in 0..60 {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            let resp = self.http_client
                .get(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await
                .map_err(|e| ToolError::Execution(format!("Fal.ai poll error: {}", e)))?;

            let body: serde_json::Value = resp.json().await
                .map_err(|e| ToolError::Execution(format!("Fal.ai poll response error: {}", e)))?;

            match body["status"].as_str() {
                Some("COMPLETED") => {
                    let image_url = body["images"][0]["url"].as_str()
                        .ok_or_else(|| ToolError::Execution("No image URL in Fal.ai response".to_string()))?;
                    return Ok(image_url.to_string());
                }
                Some("FAILED") | Some("FAILURE") => {
                    return Err(ToolError::Execution("Fal.ai job failed".to_string()));
                }
                _ => continue,
            }
        }

        Err(ToolError::Execution("Fal.ai timeout".to_string()))
    }

    async fn fal_upscale(&self, image_url: &str) -> Result<String, ToolError> {
        let api_key = self.fal_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("FAL_API_KEY not set".to_string()))?;

        let payload = serde_json::json!({
            "image_url": image_url,
            "scale": 2
        });

        let resp = self.http_client
            .post(FAL_CLARITY_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Upscaler request error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Upscaler response error: {}", e)))?;

        let request_id = body["request_id"].as_str()
            .ok_or_else(|| ToolError::Execution("No request_id in upscaler response".to_string()))?;

        for _ in 0..30 {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            let poll_url = format!("{}/results?request_id={}", FAL_CLARITY_URL, request_id);
            let resp = self.http_client
                .get(&poll_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await
                .map_err(|e| ToolError::Execution(e.to_string()))?;

            let body: serde_json::Value = resp.json().await.map_err(|e| ToolError::Execution(e.to_string()))?;

            if body["status"] == "COMPLETED" {
                let upscaled_url = body["images"][0]["url"].as_str()
                    .ok_or_else(|| ToolError::Execution("No upscaled image URL".to_string()))?;
                return Ok(upscaled_url.to_string());
            }
        }

        Err(ToolError::Execution("Upscaler timeout".to_string()))
    }

    async fn generate_fal(&self, prompt: &str, size: ImageSize) -> Result<String, ToolError> {
        let request_id = self.fal_request_image(prompt, size, 50, 4.5).await?;
        let image_url = self.fal_poll_result(&request_id).await?;
        let upscaled_url = self.fal_upscale(&image_url).await?;
        Ok(upscaled_url)
    }
}

// =============================================================================
// Pollinations — Free, no API key
// =============================================================================

impl ImageGenerationTool {
    async fn generate_pollinations(&self, prompt: &str, width: u32, height: u32, seed: Option<u32>) -> Result<String, ToolError> {
        let encoded_prompt = urlencoding::encode(prompt);
        let mut url = format!(
            "{}/{}?width={}&height={}&nologo=true&private=true",
            POLLINATIONS_URL, encoded_prompt, width, height
        );
        if let Some(s) = seed {
            url.push_str(&format!("&seed={}", s));
        }

        // Pollinations returns the image directly
        let resp = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Pollinations request error: {}", e)))?;

        if !resp.status().is_success() {
            return Err(ToolError::Execution(format!("Pollinations error: {}", resp.status())));
        }

        // Save the image to a temp file
        let bytes = resp.bytes().await
            .map_err(|e| ToolError::Execution(format!("Pollinations response error: {}", e)))?;

        let output_path = format!("pollinations_{}.png", uuid::Uuid::new_v4());
        tokio::fs::write(&output_path, bytes).await
            .map_err(|e| ToolError::Execution(format!("Failed to save image: {}", e)))?;

        Ok(output_path)
    }
}

// =============================================================================
// OpenAI DALL-E 3
// =============================================================================

impl ImageGenerationTool {
    async fn generate_openai(&self, prompt: &str, size: &str, quality: &str, style: &str) -> Result<String, ToolError> {
        let api_key = self.openai_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("OPENAI_API_KEY not set".to_string()))?;

        let payload = serde_json::json!({
            "model": "dall-e-3",
            "prompt": prompt,
            "n": 1,
            "size": size,
            "quality": quality,
            "style": style,
            "response_format": "url"
        });

        let resp = self.http_client
            .post(OPENAI_DALLE_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenAI DALL-E request error: {}", e)))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!("OpenAI DALL-E API error: {}", err_text)));
        }

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("OpenAI response error: {}", e)))?;

        let image_url = body["data"][0]["url"].as_str()
            .ok_or_else(|| ToolError::Execution("No image URL in OpenAI response".to_string()))?;

        // Download the image
        let img_resp = self.http_client
            .get(image_url)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Image download error: {}", e)))?;

        let bytes = img_resp.bytes().await
            .map_err(|e| ToolError::Execution(format!("Image download error: {}", e)))?;

        let output_path = format!("openai_dalle_{}.png", uuid::Uuid::new_v4());
        tokio::fs::write(&output_path, bytes).await
            .map_err(|e| ToolError::Execution(format!("Failed to save image: {}", e)))?;

        Ok(output_path)
    }
}

// =============================================================================
// Stability AI SD3
// =============================================================================

impl ImageGenerationTool {
    async fn generate_stability(&self, prompt: &str, aspect_ratio: &str) -> Result<String, ToolError> {
        let api_key = self.stability_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("STABILITY_API_KEY not set".to_string()))?;

        let form = reqwest::multipart::Form::new()
            .text("prompt", prompt.to_string())
            .text("aspect_ratio", aspect_ratio.to_string())
            .text("output_format", "png");

        let resp = self.http_client
            .post(STABILITY_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Accept", "image/*")
            .multipart(form)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Stability AI request error: {}", e)))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!("Stability AI API error: {}", err_text)));
        }

        let bytes = resp.bytes().await
            .map_err(|e| ToolError::Execution(format!("Stability AI response error: {}", e)))?;

        let output_path = format!("stability_sd3_{}.png", uuid::Uuid::new_v4());
        tokio::fs::write(&output_path, bytes).await
            .map_err(|e| ToolError::Execution(format!("Failed to save image: {}", e)))?;

        Ok(output_path)
    }
}

// =============================================================================
// Tool interface
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct ImageGenParams {
    pub prompt: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub image_size: Option<String>,
    #[serde(default = "default_steps")]
    pub num_inference_steps: u32,
    #[serde(default = "default_guidance")]
    pub guidance_scale: f32,
    #[serde(default = "default_num")]
    pub num_images: u32,
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_provider() -> String { "pollinations".to_string() }
fn default_steps() -> u32 { 50 }
fn default_guidance() -> f32 { 4.5 }
fn default_num() -> u32 { 1 }
fn default_quality() -> String { "standard".to_string() }
fn default_style() -> String { "vivid".to_string() }

#[async_trait]
impl Tool for ImageGenerationTool {
    fn name(&self) -> &str { "image_generate" }

    fn description(&self) -> &str {
        "Generate images from text prompts. Supports pollinations (free, no API key), fal (FLUX 2 Pro + upscale), openai (DALL-E 3), stability (SD3)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Image description/prompt" },
                "provider": {
                    "type": "string",
                    "enum": ["pollinations", "fal", "openai", "stability"],
                    "default": "pollinations",
                    "description": "Image generation provider. pollinations is free and requires no API key."
                },
                "image_size": {
                    "type": "string",
                    "enum": ["landscape_16_9", "portrait_9_16", "square_1_1", "landscape_4_3", "1024x1024", "1792x1024", "1024x1792"],
                    "default": "landscape_16_9",
                    "description": "Image size. For OpenAI: 1024x1024, 1792x1024, 1024x1792. For others: landscape_16_9, portrait_9_16, square_1_1, landscape_4_3."
                },
                "quality": {
                    "type": "string",
                    "enum": ["standard", "hd"],
                    "default": "standard",
                    "description": "Quality (OpenAI DALL-E only)"
                },
                "style": {
                    "type": "string",
                    "enum": ["vivid", "natural"],
                    "default": "vivid",
                    "description": "Style (OpenAI DALL-E only)"
                },
                "num_inference_steps": { "type": "integer", "default": 50, "description": "Inference steps (fal only)" },
                "guidance_scale": { "type": "number", "default": 4.5, "description": "Guidance scale (fal only)" },
                "num_images": { "type": "integer", "default": 1, "description": "Number of images (max 1 for DALL-E 3)" }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: ImageGenParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let size_str = params.image_size.as_deref().unwrap_or("landscape_16_9");

        let result = match params.provider.as_str() {
            "pollinations" => {
                let (width, height) = match size_str {
                    "portrait_9_16" => (768, 1344),
                    "square_1_1" => (1024, 1024),
                    "landscape_4_3" => (1024, 768),
                    _ => (1344, 768),
                };
                self.generate_pollinations(&params.prompt, width, height, None).await?
            }
            "fal" => {
                if self.fal_api_key.is_none() {
                    return Err(ToolError::Execution("FAL_API_KEY not configured".to_string()));
                }
                let size = match size_str {
                    "portrait_9_16" => ImageSize::Portrait9x16,
                    "square_1_1" => ImageSize::Square1x1,
                    "landscape_4_3" => ImageSize::Landscape4x3,
                    _ => ImageSize::Landscape16x9,
                };
                self.generate_fal(&params.prompt, size).await?
            }
            "openai" => {
                if self.openai_api_key.is_none() {
                    return Err(ToolError::Execution("OPENAI_API_KEY not configured".to_string()));
                }
                let dalle_size = match size_str {
                    "square_1_1" | "1024x1024" => "1024x1024",
                    "portrait_9_16" | "1024x1792" => "1024x1792",
                    _ => "1792x1024",
                };
                self.generate_openai(&params.prompt, dalle_size, &params.quality, &params.style).await?
            }
            "stability" => {
                if self.stability_api_key.is_none() {
                    return Err(ToolError::Execution("STABILITY_API_KEY not configured".to_string()));
                }
                let aspect = match size_str {
                    "portrait_9_16" => "9:16",
                    "square_1_1" => "1:1",
                    "landscape_4_3" => "4:3",
                    _ => "16:9",
                };
                self.generate_stability(&params.prompt, aspect).await?
            }
            _ => return Err(ToolError::InvalidArgs(format!("Unknown image provider: {}", params.provider))),
        };

        // 判断结果是 URL 还是本地路径
        let is_url = result.starts_with("http://") || result.starts_with("https://");

        Ok(json!({
            "success": true,
            "output": result,
            "is_url": is_url,
            "provider": params.provider,
            "prompt": params.prompt
        }).to_string())
    }
}
