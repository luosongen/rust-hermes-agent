//! MixtureOfAgentsTool — 多 LLM 并行聚合
//!
//! 调用多个 reference models 生成多样化响应，通过 aggregator model 合成最终答案。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Clone)]
pub struct MixtureOfAgentsTool {
    http_client: reqwest::Client,
    openrouter_api_key: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MoAConfig {
    #[serde(default)]
    pub reference_models: Vec<String>,
    #[serde(default = "default_aggregator")]
    pub aggregator_model: String,
    #[serde(default = "default_ref_temp")]
    pub reference_temperature: f32,
    #[serde(default = "default_agg_temp")]
    pub aggregator_temperature: f32,
    #[serde(default = "default_min_refs")]
    pub min_successful_references: usize,
}

fn default_aggregator() -> String { "anthropic/claude-opus-4-5-sonnet-20241022".to_string() }
fn default_ref_temp() -> f32 { 0.7 }
fn default_agg_temp() -> f32 { 0.3 }
fn default_min_refs() -> usize { 2 }

impl Default for MoAConfig {
    fn default() -> Self {
        Self {
            reference_models: vec![
                "anthropic/claude-opus-4-5-sonnet-20241022".to_string(),
                "google/gemini-2.5-pro-preview-06-05".to_string(),
                "openai/gpt-5-pro".to_string(),
                "deepseek/deepseek-v3".to_string(),
            ],
            aggregator_model: default_aggregator(),
            reference_temperature: default_ref_temp(),
            aggregator_temperature: default_agg_temp(),
            min_successful_references: default_min_refs(),
        }
    }
}

impl Default for MixtureOfAgentsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MixtureOfAgentsTool {
    pub fn new() -> Self {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .expect("OPENROUTER_API_KEY not set");
        Self {
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .expect("HTTP client"),
            openrouter_api_key: api_key,
        }
    }

    pub fn with_api_key(mut self, key: String) -> Self {
        self.openrouter_api_key = key;
        self
    }

    async fn call_openrouter(&self, model: &str, prompt: &str, temperature: f32) -> Result<String, ToolError> {
        let payload = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": temperature
        });

        let resp = self.http_client
            .post(OPENROUTER_URL)
            .header("Authorization", format!("Bearer {}", self.openrouter_api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenRouter API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("OpenRouter response error: {}", e)))?;

        body["choices"][0]["message"]["content"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ToolError::Execution("No content in OpenRouter response".to_string()))
    }

    async fn call_reference_models(
        &self,
        prompt: &str,
        models: &[String],
        temperature: f32,
    ) -> Vec<(String, String)> {
        let mut handles = Vec::new();
        for model in models {
            let model = model.clone();
            let prompt = prompt.to_string();
            let client = self.http_client.clone();
            let api_key = self.openrouter_api_key.clone();
            handles.push(tokio::spawn(async move {
                let payload = serde_json::json!({
                    "model": model,
                    "messages": [{"role": "user", "content": prompt}],
                    "temperature": temperature
                });
                let resp = client.post(OPENROUTER_URL)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send().await;
                match resp {
                    Ok(r) => {
                        let body: serde_json::Value = r.json().await.ok()?;
                        body["choices"][0]["message"]["content"].as_str()
                            .map(|s| (model, s.to_string()))
                    }
                    Err(_) => None
                }
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok(Some(result)) = handle.await {
                results.push(result);
            }
        }
        results
    }
}

#[derive(Debug, Deserialize)]
pub struct MoAParams {
    pub prompt: String,
    #[serde(default)]
    pub reference_models: Option<Vec<String>>,
    #[serde(default)]
    pub aggregator_model: Option<String>,
}

#[async_trait]
impl Tool for MixtureOfAgentsTool {
    fn name(&self) -> &str { "mixture_of_agents" }

    fn description(&self) -> &str {
        "Solve complex queries using multiple frontier LLMs in parallel, synthesized by an aggregator."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "reference_models": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Override default reference models"
                },
                "aggregator_model": {
                    "type": "string",
                    "description": "Override default aggregator model"
                }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: MoAParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let config = MoAConfig {
            reference_models: params.reference_models.unwrap_or_else(|| MoAConfig::default().reference_models),
            aggregator_model: params.aggregator_model.unwrap_or_else(|| MoAConfig::default().aggregator_model),
            ..Default::default()
        };

        // Step 1: 并行调用 reference models
        let references = self.call_reference_models(
            &params.prompt,
            &config.reference_models,
            config.reference_temperature
        ).await;

        if references.len() < config.min_successful_references {
            return Err(ToolError::Execution(format!(
                "Only {} reference models succeeded, need {}",
                references.len(), config.min_successful_references
            )));
        }

        // Step 2: 构建 aggregator prompt
        let aggregator_prompt = format!(
            "You are a synthesis AI. Combine the following {} reference responses into a single coherent answer.\n\n{}\n\nProvide your synthesized answer:",
            references.len(),
            references.iter().enumerate().map(|(i, (_, r))| format!("[Reference {}]\n{}\n", i + 1, r)).collect::<String>()
        );

        // Step 3: 调用 aggregator
        let answer = self.call_openrouter(&config.aggregator_model, &aggregator_prompt, config.aggregator_temperature).await?;

        Ok(json!({
            "success": true,
            "answer": answer,
            "reference_count": references.len(),
            "references": references.iter().map(|(m, r)| {
                json!({ "model": m, "excerpt": &r[..r.len().min(200)] })
            }).collect::<Vec<_>>(),
            "aggregator": config.aggregator_model
        }).to_string())
    }
}
