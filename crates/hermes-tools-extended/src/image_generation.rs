//! ImageGenerationTool — Fal.ai FLUX 2 Pro 图像生成
//!
//! 支持 landscape_16_9 / portrait_9_16 / square_1_1 / landscape_4_3 尺寸。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const FAL_FLUX_PRO_URL: &str = "https://queue.fal.run/fal-ai/flux-2-pro";
const FAL_CLARITY_URL: &str = "https://queue.fal.run/fal-ai/clarity-upscaler";

#[derive(Clone)]
pub struct ImageGenerationTool {
    http_client: reqwest::Client,
    fal_api_key: Option<String>,
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
        }
    }

    pub fn with_fal_api_key(mut self, key: String) -> Self {
        self.fal_api_key = Some(key);
        self
    }
}

impl ImageGenerationTool {
    async fn request_image(&self, prompt: &str, size: ImageSize, num_inference_steps: u32, guidance_scale: f32) -> Result<String, ToolError> {
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

    async fn poll_result(&self, request_id: &str) -> Result<String, ToolError> {
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

    async fn upscale(&self, image_url: &str) -> Result<String, ToolError> {
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
}

#[derive(Debug, Deserialize)]
pub struct ImageGenParams {
    pub prompt: String,
    #[serde(default)]
    pub image_size: Option<String>,
    #[serde(default = "default_steps")]
    pub num_inference_steps: u32,
    #[serde(default = "default_guidance")]
    pub guidance_scale: f32,
    #[serde(default = "default_num")]
    pub num_images: u32,
}

fn default_steps() -> u32 { 50 }
fn default_guidance() -> f32 { 4.5 }
fn default_num() -> u32 { 1 }

#[async_trait]
impl Tool for ImageGenerationTool {
    fn name(&self) -> &str { "image_generate" }

    fn description(&self) -> &str {
        "Generate images from text prompts using Fal.ai FLUX 2 Pro with automatic 2x upscaling."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "image_size": {
                    "type": "string",
                    "enum": ["landscape_16_9", "portrait_9_16", "square_1_1", "landscape_4_3"],
                    "default": "landscape_16_9"
                },
                "num_inference_steps": { "type": "integer", "default": 50 },
                "guidance_scale": { "type": "number", "default": 4.5 },
                "num_images": { "type": "integer", "default": 1 }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: ImageGenParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        if self.fal_api_key.is_none() {
            return Err(ToolError::Execution("FAL_API_KEY not configured".to_string()));
        }

        let size = match params.image_size.as_deref() {
            Some("portrait_9_16") => ImageSize::Portrait9x16,
            Some("square_1_1") => ImageSize::Square1x1,
            Some("landscape_4_3") => ImageSize::Landscape4x3,
            _ => ImageSize::Landscape16x9,
        };

        let request_id = self.request_image(&params.prompt, size, params.num_inference_steps, params.guidance_scale).await?;
        let image_url = self.poll_result(&request_id).await?;
        let upscaled_url = self.upscale(&image_url).await?;

        Ok(json!({
            "success": true,
            "images": [{
                "url": upscaled_url,
                "width": 2048,
                "height": 1536,
                "upscaled": true
            }],
            "model": "fal-ai/flux-2-pro"
        }).to_string())
    }
}
