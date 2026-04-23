//! TextToSpeechTool — 文本转语音
//!
//! 支持多提供商：
//! - **edge-tts**（默认，免费）— Microsoft Edge 在线 TTS，无需 API Key
//! - **openai** — OpenAI TTS API（tts-1, tts-1-hd）
//! - **elevenlabs** — ElevenLabs API
//!
//! 输出格式：MP3（默认），edge-tts 还支持 WAV、OGG。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

const OPENAI_TTS_URL: &str = "https://api.openai.com/v1/audio/speech";
const ELEVENLABS_URL: &str = "https://api.elevenlabs.io/v1/text-to-speech";

#[derive(Clone)]
pub struct TextToSpeechTool {
    http_client: reqwest::Client,
    openai_api_key: Option<String>,
    elevenlabs_api_key: Option<String>,
}

impl Default for TextToSpeechTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TextToSpeechTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("HTTP client"),
            openai_api_key: std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))
                .ok(),
            elevenlabs_api_key: std::env::var("ELEVENLABS_API_KEY").ok(),
        }
    }

    pub fn with_openai_key(mut self, key: String) -> Self {
        self.openai_api_key = Some(key);
        self
    }

    pub fn with_elevenlabs_key(mut self, key: String) -> Self {
        self.elevenlabs_api_key = Some(key);
        self
    }

    /// Edge-TTS：调用 edge-tts Python CLI
    async fn synthesize_edge_tts(
        &self,
        text: &str,
        voice: &str,
        output_path: &str,
    ) -> Result<String, ToolError> {
        // 检测 edge-tts 是否可用
        let check = tokio::process::Command::new("edge-tts")
            .arg("--version")
            .output()
            .await;

        if check.is_err() || !check.unwrap().status.success() {
            return Err(ToolError::MissingEnv(
                "edge-tts not found. Install with: pip install edge-tts".to_string(),
            ));
        }

        let mut cmd = tokio::process::Command::new("edge-tts");
        cmd.arg("--voice").arg(voice)
            .arg("--text").arg(text)
            .arg("--write-media").arg(output_path);

        let output = cmd.output().await
            .map_err(|e| ToolError::Execution(format!("edge-tts error: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!("edge-tts failed: {}", stderr)));
        }

        Ok(output_path.to_string())
    }

    /// OpenAI TTS API
    async fn synthesize_openai(
        &self,
        text: &str,
        voice: &str,
        model: &str,
        output_path: &str,
    ) -> Result<String, ToolError> {
        let api_key = self.openai_api_key.as_ref()
            .ok_or_else(|| ToolError::MissingEnv("OPENAI_API_KEY not set".to_string()))?;

        let payload = serde_json::json!({
            "model": model,
            "input": text,
            "voice": voice,
            "response_format": "mp3"
        });

        let resp = self.http_client
            .post(OPENAI_TTS_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenAI TTS request error: {}", e)))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!("OpenAI TTS API error: {}", err_text)));
        }

        let bytes = resp.bytes().await
            .map_err(|e| ToolError::Execution(format!("OpenAI TTS response error: {}", e)))?;

        tokio::fs::write(output_path, bytes).await
            .map_err(|e| ToolError::Execution(format!("Failed to write audio file: {}", e)))?;

        Ok(output_path.to_string())
    }

    /// ElevenLabs TTS API
    async fn synthesize_elevenlabs(
        &self,
        text: &str,
        voice_id: &str,
        model: &str,
        output_path: &str,
    ) -> Result<String, ToolError> {
        let api_key = self.elevenlabs_api_key.as_ref()
            .ok_or_else(|| ToolError::MissingEnv("ELEVENLABS_API_KEY not set".to_string()))?;

        let url = format!("{}/{}", ELEVENLABS_URL, voice_id);

        let payload = serde_json::json!({
            "text": text,
            "model_id": model,
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.5
            }
        });

        let resp = self.http_client
            .post(&url)
            .header("xi-api-key", api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("ElevenLabs request error: {}", e)))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!("ElevenLabs API error: {}", err_text)));
        }

        let bytes = resp.bytes().await
            .map_err(|e| ToolError::Execution(format!("ElevenLabs response error: {}", e)))?;

        tokio::fs::write(output_path, bytes).await
            .map_err(|e| ToolError::Execution(format!("Failed to write audio file: {}", e)))?;

        Ok(output_path.to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct TtsParams {
    pub text: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_voice")]
    pub voice: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_output")]
    pub output_path: String,
}

fn default_provider() -> String { "edge-tts".to_string() }
fn default_voice() -> String { "zh-CN-XiaoxiaoNeural".to_string() }
fn default_model() -> String { "tts-1".to_string() }
fn default_output() -> String { "output.mp3".to_string() }

#[async_trait]
impl Tool for TextToSpeechTool {
    fn name(&self) -> &str { "text_to_speech" }

    fn description(&self) -> &str {
        "Convert text to speech audio. Supports edge-tts (free, default), OpenAI TTS, and ElevenLabs. \
         For edge-tts, install: pip install edge-tts"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to convert to speech"
                },
                "provider": {
                    "type": "string",
                    "enum": ["edge-tts", "openai", "elevenlabs"],
                    "default": "edge-tts",
                    "description": "TTS provider. edge-tts is free and requires 'pip install edge-tts'"
                },
                "voice": {
                    "type": "string",
                    "default": "zh-CN-XiaoxiaoNeural",
                    "description": "Voice ID. edge-tts examples: zh-CN-XiaoxiaoNeural, en-US-AriaNeural. OpenAI: alloy, echo, fable, onyx, nova, shimmer. ElevenLabs: voice ID"
                },
                "model": {
                    "type": "string",
                    "default": "tts-1",
                    "description": "Model (OpenAI: tts-1, tts-1-hd; ElevenLabs: eleven_multilingual_v2)"
                },
                "output_path": {
                    "type": "string",
                    "default": "output.mp3",
                    "description": "Output audio file path"
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: TtsParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let output_path = if PathBuf::from(&params.output_path).is_absolute() {
            params.output_path
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&params.output_path)
                .to_string_lossy()
                .to_string()
        };

        let result_path = match params.provider.as_str() {
            "edge-tts" => {
                self.synthesize_edge_tts(&params.text, &params.voice, &output_path).await?
            }
            "openai" => {
                if self.openai_api_key.is_none() {
                    return Err(ToolError::Execution("OPENAI_API_KEY not configured".to_string()));
                }
                self.synthesize_openai(&params.text, &params.voice, &params.model, &output_path).await?
            }
            "elevenlabs" => {
                if self.elevenlabs_api_key.is_none() {
                    return Err(ToolError::Execution("ELEVENLABS_API_KEY not configured".to_string()));
                }
                self.synthesize_elevenlabs(&params.text, &params.voice, &params.model, &output_path).await?
            }
            _ => return Err(ToolError::InvalidArgs(format!("Unknown TTS provider: {}", params.provider))),
        };

        // 获取文件大小
        let metadata = tokio::fs::metadata(&result_path).await.ok();
        let file_size = metadata.map(|m| m.len());

        Ok(json!({
            "success": true,
            "output_path": result_path,
            "provider": params.provider,
            "voice": params.voice,
            "file_size_bytes": file_size,
            "text_length": params.text.len()
        }).to_string())
    }
}
