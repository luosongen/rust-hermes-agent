//! LLM-based message summarization

use crate::compression_config::{SummarizerConfig, SummarizerProvider};
use crate::compression_error::CompressionError;
use crate::session::Message;
use reqwest::Client;

/// Summarizer for generating message summaries
pub struct Summarizer {
    config: SummarizerConfig,
    http_client: Client,
}

impl Summarizer {
    pub fn new(config: SummarizerConfig) -> Self {
        Self {
            config,
            http_client: Client::new(),
        }
    }

    /// Generate a summary for a list of messages
    pub async fn summarize(
        &self,
        messages: &[Message],
        _max_tokens: usize,
    ) -> Result<String, CompressionError> {
        match self.config.provider {
            SummarizerProvider::OpenAi => {
                self.summarize_openai(messages).await
            }
            SummarizerProvider::Ollama => {
                self.summarize_ollama(messages).await
            }
        }
    }

    /// Generate embedding vector for text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, CompressionError> {
        match self.config.provider {
            SummarizerProvider::OpenAi => {
                self.embed_openai(text).await
            }
            SummarizerProvider::Ollama => {
                self.embed_ollama(text).await
            }
        }
    }

    async fn summarize_openai(
        &self,
        messages: &[Message],
    ) -> Result<String, CompressionError> {
        // Build conversation context
        let context = messages
            .iter()
            .filter_map(|m| {
                let role = &m.role;
                let content = m.content.as_deref().unwrap_or("");
                if content.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", role, content))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Summarize the following conversation concisely, capturing the key points and any important details:\n\n{}\n\nSummary:",
            context
        );

        // For OpenAI, we use the chat completions API
        // The API key should come from environment or config
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| CompressionError::Config("OPENAI_API_KEY not set".into()))?;

        let request = serde_json::json!({
            "model": self.config.model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 500,
            "temperature": 0.3
        });

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CompressionError::LlmApi(format!(
                "OpenAI API error: {} - {}",
                status, body
            )));
        }

        #[derive(serde::Deserialize)]
        struct OpenAiResponse {
            choices: Vec<Choice>,
        }

        #[derive(serde::Deserialize)]
        struct Choice {
            message: MessageContent,
        }

        #[derive(serde::Deserialize)]
        struct MessageContent {
            content: String,
        }

        let resp: OpenAiResponse = response
            .json()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        resp.choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| CompressionError::LlmApi("No choices in response".into()))
    }

    async fn summarize_ollama(
        &self,
        messages: &[Message],
    ) -> Result<String, CompressionError> {
        let ollama_url = self.config.ollama_url.as_ref()
            .ok_or_else(|| CompressionError::Config("Ollama URL not configured".into()))?;

        let context = messages
            .iter()
            .filter_map(|m| {
                let role = &m.role;
                let content = m.content.as_deref().unwrap_or("");
                if content.is_empty() {
                    None
                } else {
                    Some(format!("{}: {}", role, content))
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let request = serde_json::json!({
            "model": self.config.model,
            "prompt": format!(
                "Summarize the following conversation concisely:\n\n{}\n\nSummary:",
                context
            ),
            "stream": false
        });

        let response = self.http_client
            .post(format!("{}/api/generate", ollama_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        if !response.status().is_success() {
            return Err(CompressionError::LlmApi(format!(
                "Ollama returned status: {}",
                response.status()
            )));
        }

        #[derive(serde::Deserialize)]
        struct OllamaResponse {
            response: String,
        }

        let ollama_resp: OllamaResponse = response
            .json()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        Ok(ollama_resp.response)
    }

    async fn embed_openai(&self, text: &str) -> Result<Vec<f32>, CompressionError> {
        // Use OpenAI embeddings API
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| CompressionError::Config("OPENAI_API_KEY not set".into()))?;

        let request = serde_json::json!({
            "model": "text-embedding-ada-002",
            "input": text
        });

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        #[derive(serde::Deserialize)]
        struct OpenAiEmbedResponse {
            data: Vec<EmbedData>,
        }

        #[derive(serde::Deserialize)]
        struct EmbedData {
            embedding: Vec<f32>,
        }

        let embed_resp: OpenAiEmbedResponse = response
            .json()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        embed_resp.data
            .first()
            .map(|d| d.embedding.clone())
            .ok_or_else(|| CompressionError::LlmApi("No embedding in response".into()))
    }

    async fn embed_ollama(&self, text: &str) -> Result<Vec<f32>, CompressionError> {
        let ollama_url = self.config.ollama_url.as_ref()
            .ok_or_else(|| CompressionError::Config("Ollama URL not configured".into()))?;

        #[derive(serde::Serialize)]
        struct OllamaEmbedRequest<'a> {
            model: &'a str,
            input: &'a str,
        }

        let request = OllamaEmbedRequest {
            model: &self.config.model,
            input: text,
        };

        let response = self.http_client
            .post(format!("{}/api/embeddings", ollama_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        #[derive(serde::Deserialize)]
        struct OllamaEmbedResponse {
            embedding: Vec<f32>,
        }

        let embed_resp: OllamaEmbedResponse = response
            .json()
            .await
            .map_err(|e| CompressionError::LlmApi(e.to_string()))?;

        Ok(embed_resp.embedding)
    }
}