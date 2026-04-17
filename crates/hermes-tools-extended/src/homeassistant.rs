//! HomeAssistantTool — 控制 Home Assistant 智能家居设备
//!
//! 支持 list_entities / get_state / list_services / call_service 四个 action。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;

lazy_static::lazy_static! {
    static ref ENTITY_ID_RE: Regex = Regex::new(r"^[a-z_][a-z0-9_]*\.[a-z0-9_]+$").unwrap();
    static ref SERVICE_NAME_RE: Regex = Regex::new(r"^[a-z][a-z0-9_]*$").unwrap();
    static ref BLOCKED_DOMAINS: HashSet<&'static str> = [
        "shell_command", "command_line", "python_script", "pyscript", "hassio", "rest_command"
    ].into_iter().collect();
}

#[derive(Clone)]
pub struct HomeAssistantTool {
    http_client: reqwest::Client,
    hass_url: String,
    hass_token: Option<String>,
}

impl Default for HomeAssistantTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeAssistantTool {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
            hass_url: std::env::var("HASS_URL").unwrap_or_else(|_| "http://localhost:8123".to_string()),
            hass_token: std::env::var("HASS_TOKEN").ok(),
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.hass_url = url;
        self
    }

    pub fn with_token(mut self, token: String) -> Self {
        self.hass_token = Some(token);
        self
    }

    fn validate_service_name(service: &str) -> Result<(), ToolError> {
        if !SERVICE_NAME_RE.is_match(service) {
            return Err(ToolError::InvalidArgs(format!("Invalid service name: {}", service)));
        }
        Ok(())
    }

    async fn ha_get(&self, path: &str) -> Result<serde_json::Value, ToolError> {
        let url = format!("{}/api/{}", self.hass_url.trim_end_matches('/'), path);
        let mut req = self.http_client.get(&url);
        if let Some(token) = &self.hass_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.send().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant API error: {}", e)))?;
        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant response error: {}", e)))?;
        Ok(body)
    }

    async fn ha_post(&self, path: &str, data: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let url = format!("{}/api/{}", self.hass_url.trim_end_matches('/'), path);
        let mut req = self.http_client.post(&url);
        if let Some(token) = &self.hass_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }
        let resp = req.json(&data).send().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant POST error: {}", e)))?;
        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("HomeAssistant POST response error: {}", e)))?;
        Ok(body)
    }
}

pub fn validate_entity_id(entity_id: &str) -> Result<(), ToolError> {
    if !ENTITY_ID_RE.is_match(entity_id) {
        return Err(ToolError::InvalidArgs(format!("Invalid entity_id format: {}", entity_id)));
    }
    let domain = entity_id.split('.').next().unwrap();
    if BLOCKED_DOMAINS.contains(domain) {
        return Err(ToolError::InvalidArgs(format!("Blocked domain: {}", domain)));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct HaParams {
    pub action: String,
    pub domain: Option<String>,
    pub area: Option<String>,
    pub entity_id: Option<String>,
    pub service: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[async_trait]
impl Tool for HomeAssistantTool {
    fn name(&self) -> &str { "homeassistant" }

    fn description(&self) -> &str {
        "Control Home Assistant devices. Supports ha_list_entities, ha_get_state, ha_list_services, ha_call_service."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {
                    "properties": {
                        "action": { "const": "ha_list_entities" },
                        "domain": { "type": "string" },
                        "area": { "type": "string" }
                    },
                    "required": ["action"]
                },
                {
                    "properties": {
                        "action": { "const": "ha_get_state" },
                        "entity_id": { "type": "string" }
                    },
                    "required": ["action", "entity_id"]
                },
                {
                    "properties": {
                        "action": { "const": "ha_list_services" },
                        "domain": { "type": "string" }
                    },
                    "required": ["action"]
                },
                {
                    "properties": {
                        "action": { "const": "ha_call_service" },
                        "domain": { "type": "string" },
                        "service": { "type": "string" },
                        "entity_id": { "type": "string" },
                        "data": { "type": "object" }
                    },
                    "required": ["action", "domain", "service", "entity_id"]
                }
            ]
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: HaParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params.action.as_str() {
            "ha_list_entities" => {
                let state: serde_json::Value = self.ha_get("states").await?;
                let state = state.as_array().ok_or_else(|| ToolError::Execution("Expected array from /api/states".to_string()))?;
                let filtered: Vec<_> = state.iter().filter(|s| {
                    if let (Some(dom), Some(area)) = (params.domain.as_ref(), params.area.as_ref()) {
                        s["entity_id"].as_str().map(|e: &str| e.starts_with(&format!("{}.", dom))).unwrap_or(false)
                            && s["attributes"]["area_id"].as_str() == Some(area)
                    } else if let Some(dom) = params.domain.as_ref() {
                        s["entity_id"].as_str().map(|e: &str| e.starts_with(&format!("{}.", dom))).unwrap_or(false)
                    } else {
                        true
                    }
                }).collect();
                Ok(json!({ "entities": filtered }).to_string())
            }
            "ha_get_state" => {
                let entity_id = params.entity_id.as_ref().unwrap();
                validate_entity_id(entity_id)?;
                let state = self.ha_get(&format!("states/{}", entity_id)).await?;
                Ok(json!({ "state": state }).to_string())
            }
            "ha_list_services" => {
                let services: serde_json::Value = self.ha_get("services").await?;
                if let Some(domain) = params.domain.as_ref() {
                    let filtered = services.get(domain).cloned().unwrap_or(serde_json::Value::Null);
                    Ok(json!({ "services": filtered }).to_string())
                } else {
                    Ok(json!({ "services": services }).to_string())
                }
            }
            "ha_call_service" => {
                let domain = params.domain.as_ref().unwrap();
                let service = params.service.as_ref().unwrap();
                let entity_id = params.entity_id.as_ref().unwrap();
                validate_entity_id(entity_id)?;
                Self::validate_service_name(service)?;

                let data = serde_json::json!({
                    "entity_id": entity_id,
                    "data": params.data.unwrap_or(serde_json::Value::Null)
                });

                let result = self.ha_post(&format!("services/{}", domain), data).await?;
                Ok(json!({ "result": result }).to_string())
            }
            _ => Err(ToolError::InvalidArgs(format!("Unknown action: {}", params.action))),
        }
    }
}
