//! MixtureOfAgentsTool — 多 LLM 并行聚合工具
//!
//! 调用多个 reference models 生成多样化响应，通过 aggregator model 合成最终答案。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Clone)]
pub struct MixtureOfAgentsTool {
    http_client: Client,
    openrouter_api_key: Option<String>,
    config: MoAConfig,
}

#[derive(Debug, Clone)]
pub struct MoAConfig {
    pub reference_models: Vec<String>,
    pub aggregator_model: String,
    pub reference_temperature: f32,
    pub aggregator_temperature: f32,
    pub min_successful_references: usize,
}

impl Default for MoAConfig {
    fn default() -> Self {
        Self {
            reference_models: vec![
                "anthropic/claude-opus-4-5-sonnet-20241022".to_string(),
                "google/gemini-2.5-pro-preview-06-05".to_string(),
                "openai/gpt-5-pro".to_string(),
                "deepseek/deepseek-v3".to_string(),
            ],
            aggregator_model: "anthropic/claude-opus-4-5-sonnet-20241022".to_string(),
            reference_temperature: 0.7,
            aggregator_temperature: 0.3,
            min_successful_references: 2,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum MoAParams {
    Generate {
        prompt: String,
        #[serde(default)]
        reference_models: Option<Vec<String>>,
        #[serde(default)]
        aggregator_model: Option<String>,
    },
}

impl MixtureOfAgentsTool {
    pub fn new() -> Self {
        Self {
            http_client: Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .unwrap(),
            openrouter_api_key: None,
            config: MoAConfig::default(),
        }
    }

    /// 共享的 OpenRouter API 调用（内部使用，不暴露给 Tool）
    async fn call_openrouter(
        &self,
        model: &str,
        prompt: &str,
        temperature: f32,
    ) -> Result<String, ToolError> {
        let api_key = self.openrouter_api_key.as_ref()
            .ok_or_else(|| ToolError::Execution("OpenRouter API key not configured".to_string()))?;

        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": temperature
        });

        let resp = self.http_client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("OpenRouter request failed: {}", e)))?;

        let resp_json: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Failed to parse OpenRouter response: {}", e)))?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| ToolError::Execution("Invalid OpenRouter response format".to_string()))?;

        Ok(content.to_string())
    }

    /// 并行调用 reference models
    async fn call_reference_models(
        &self,
        prompt: &str,
        models: &[String],
    ) -> Vec<(String, Result<String, String>)> {
        let reference_temp = self.config.reference_temperature;
        let mut handles = Vec::new();

        for model in models {
            let model = model.clone();
            let prompt = prompt.to_string();
            let http_client = self.http_client.clone();
            let api_key = self.openrouter_api_key.clone();
            let reference_temp = reference_temp;

            handles.push(tokio::spawn(async move {
                let api_key_val = match api_key.as_ref() {
                    Some(k) => k,
                    None => return (model, Err("OpenRouter API key not configured".to_string())),
                };

                let payload = serde_json::json!({
                    "model": model,
                    "messages": [{"role": "user", "content": prompt}],
                    "temperature": reference_temp
                });

                let resp = match http_client
                    .post(OPENROUTER_API_URL)
                    .header("Authorization", format!("Bearer {}", api_key_val))
                    .header("Content-Type", "application/json")
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return (model, Err(e.to_string())),
                };

                let resp_json: serde_json::Value = match resp.json().await {
                    Ok(v) => v,
                    Err(e) => return (model, Err(e.to_string())),
                };

                let content: String = match resp_json["choices"][0]["message"]["content"].as_str() {
                    Some(s) => s.to_string(),
                    None => "No content in response".to_string(),
                };

                (model, Ok(content))
            }));
        }

        let mut results = Vec::new();
        for handle in handles {
            if let Ok((model, result)) = handle.await {
                results.push((model, result));
            } else {
                results.push((String::new(), Err("Task panicked".to_string())));
            }
        }
        results
    }

    /// 合成最终答案
    async fn aggregate(
        &self,
        prompt: &str,
        references: &[(String, Result<String, String>)],
        aggregator: &str,
    ) -> Result<String, ToolError> {
        let mut reference_text = String::new();
        for (i, (model, content)) in references.iter().enumerate() {
            let content_str = match content {
                Ok(c) => c.as_str(),
                Err(e) => e.as_str(),
            };
            reference_text.push_str(&format!("\n\n=== Reference {} ({}):\n{}", i + 1, model, content_str));
        }

        let aggregator_prompt = format!(
            "You are a synthesizer combining multiple AI model responses into one coherent answer.\n\nOriginal question: {}\n\nReferences: {}\n\nBased on the references above, provide a comprehensive, accurate synthesis that captures the best insights from all references. Be concise but thorough.",
            prompt,
            reference_text
        );

        self.call_openrouter(aggregator, &aggregator_prompt, self.config.aggregator_temperature).await
    }
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
            "oneOf": [
                {
                    "properties": {
                        "action": { "const": "generate" },
                        "prompt": { "type": "string" },
                        "reference_models": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "aggregator_model": { "type": "string" }
                    },
                    "required": ["action", "prompt"]
                }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: MoAParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params {
            MoAParams::Generate { prompt, reference_models, aggregator_model } => {
                let reference_models = reference_models.unwrap_or_else(|| self.config.reference_models.clone());
                let aggregator_model = aggregator_model.unwrap_or_else(|| self.config.aggregator_model.clone());

                // 并行调用 reference models
                let references = self.call_reference_models(&prompt, &reference_models).await;

                // Count successful references
                let successful: Vec<_> = references.iter().filter(|(_, r)| r.is_ok()).collect();
                if successful.len() < self.config.min_successful_references {
                    return Err(ToolError::Execution(format!(
                        "Only {} reference models succeeded, minimum {} required",
                        successful.len(),
                        self.config.min_successful_references
                    )));
                }

                // 合成最终答案
                let answer = self.aggregate(&prompt, &references, &aggregator_model).await?;

                let response = json!({
                    "success": true,
                    "answer": answer,
                    "reference_count": successful.len(),
                    "references": references.iter().filter(|(_, r)| r.is_ok()).map(|(model, content)| {
                        let content = content.as_ref().unwrap();
                        json!({
                            "model": model,
                            "excerpt": if content.len() > 200 { &content[..200] } else { content }
                        })
                    }).collect::<Vec<_>>(),
                    "aggregator": aggregator_model
                });

                Ok(response.to_string())
            }
        }
    }
}
