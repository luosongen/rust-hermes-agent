//! TranscriptionTool — 语音转文字
//!
//! 支持 faster-whisper 本地运行和 Groq Whisper API。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const OPENAI_WHISPER_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

#[derive(Clone)]
pub struct TranscriptionTool {
    http_client: reqwest::Client,
    groq_api_key: Option<String>,
    openai_api_key: Option<String>,
    whisper_model_path: Option<PathBuf>,
}

impl Default for TranscriptionTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TranscriptionTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            groq_api_key: std::env::var("GROQ_API_KEY").ok(),
            openai_api_key: std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("HERMES_OPENAI_API_KEY"))
                .ok(),
            whisper_model_path: None,
        }
    }

    pub fn with_groq_api_key(mut self, key: String) -> Self {
        self.groq_api_key = Some(key);
        self
    }

    pub fn with_openai_api_key(mut self, key: String) -> Self {
        self.openai_api_key = Some(key);
        self
    }

    pub fn with_whisper_model_path(mut self, path: PathBuf) -> Self {
        self.whisper_model_path = Some(path);
        self
    }

    async fn transcribe_faster_whisper(&self, audio_path: &str, language: Option<&str>) -> Result<String, ToolError> {
        let model_path = self.whisper_model_path.clone()
            .unwrap_or_else(|| {
                PathBuf::from(
                    std::env::var("WHISPER_MODEL_PATH")
                        .unwrap_or_else(|_| "~/.cache/faster-whisper".to_string())
                )
            });

        let mut cmd = tokio::process::Command::new("whisper");
        cmd.arg("--model").arg(model_path)
           .arg("--language").arg(language.unwrap_or("auto"))
           .arg("--output_format").arg("json")
           .arg("--input").arg(audio_path);

        let output = cmd.output().await
            .map_err(|e| ToolError::Execution(format!("faster-whisper error: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!("faster-whisper failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_val: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| ToolError::Execution(format!("faster-whisper JSON parse error: {}", e)))?;

        json_val["text"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::Execution("No text in faster-whisper output".to_string()))
    }

    async fn transcribe_openai(&self, audio_path: &str, language: Option<&str>) -> Result<String, ToolError> {
        let api_key = self.openai_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("OPENAI_API_KEY not set".to_string()))?;

        let audio_bytes = tokio::fs::read(audio_path).await
            .map_err(|e| ToolError::Execution(format!("Audio file read error: {}", e)))?;

        let file_name = PathBuf::from(audio_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio.mp3".to_string());

        let mut form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(audio_bytes)
                .file_name(file_name)
                .mime_str("audio/mpeg")
                .unwrap_or_else(|_| reqwest::multipart::Part::bytes(Vec::new())))
            .text("model", "whisper-1");

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        let resp = self.http_client
            .post(OPENAI_WHISPER_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenAI Whisper API error: {}", e)))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!("OpenAI Whisper API error: {}", err_text)));
        }

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("OpenAI response error: {}", e)))?;

        body["text"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::Execution("No text in OpenAI response".to_string()))
    }

    async fn transcribe_groq(&self, audio_path: &str, language: Option<&str>) -> Result<String, ToolError> {
        let api_key = self.groq_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("GROQ_API_KEY not set".to_string()))?;

        let audio_bytes = tokio::fs::read(audio_path).await
            .map_err(|e| ToolError::Execution(format!("Audio file read error: {}", e)))?;

        let file_name = PathBuf::from(audio_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio.mp3".to_string());

        let mut form = reqwest::multipart::Form::new()
            .part("file", reqwest::multipart::Part::bytes(audio_bytes)
                .file_name(file_name)
                .mime_str("audio/mpeg")
                .unwrap_or_else(|_| reqwest::multipart::Part::bytes(Vec::new())))
            .text("model", "whisper-large-v3");

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        let resp = self.http_client
            .post(GROQ_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Groq API error: {}", e)))?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!("Groq API error: {}", err_text)));
        }

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Groq response error: {}", e)))?;

        body["text"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::Execution("No text in Groq response".to_string()))
    }
}

#[derive(Debug, Deserialize)]
pub struct TranscribeParams {
    pub audio_path: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub language: Option<String>,
}

fn default_provider() -> String { "faster-whisper".to_string() }

#[async_trait]
impl Tool for TranscriptionTool {
    fn name(&self) -> &str { "transcribe" }

    fn description(&self) -> &str {
        "Transcribe audio to text. Supports faster-whisper (local), OpenAI Whisper API, and Groq Whisper API."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "audio_path": { "type": "string", "description": "Path to audio file" },
                "provider": {
                    "type": "string",
                    "enum": ["faster-whisper", "openai", "groq"],
                    "default": "faster-whisper"
                },
                "language": { "type": "string", "description": "Language code (e.g., 'en', 'zh')" }
            },
            "required": ["audio_path"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: TranscribeParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let text = match params.provider.as_str() {
            "openai" => {
                if self.openai_api_key.is_none() {
                    return Err(ToolError::Execution("OPENAI_API_KEY not configured".to_string()));
                }
                self.transcribe_openai(&params.audio_path, params.language.as_deref()).await?
            }
            "groq" => {
                if self.groq_api_key.is_none() {
                    return Err(ToolError::Execution("GROQ_API_KEY not configured".to_string()));
                }
                self.transcribe_groq(&params.audio_path, params.language.as_deref()).await?
            }
            _ => self.transcribe_faster_whisper(&params.audio_path, params.language.as_deref()).await?,
        };

        Ok(json!({
            "success": true,
            "text": text,
            "provider": params.provider,
            "audio_path": params.audio_path
        }).to_string())
    }
}
