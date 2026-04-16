//! WebSearchTool — 网页搜索工具
//!
//! 支持 DuckDuckGo（免费）、Exa AI、Tavily、Firecrawl 等搜索后端。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

// ============================================================================
// SearchResult — 统一的搜索结果结构
// ============================================================================

/// 搜索结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    /// 结果 URL
    pub url: String,
    /// 结果标题
    pub title: String,
    /// 摘要/片段
    pub snippet: String,
    /// 完整内容（部分 provider 支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ============================================================================
// SearchProvider enum — 所有支持的搜索后端
// ============================================================================

#[derive(Clone)]
pub enum SearchProvider {
    DuckDuckGo(DuckDuckGoProvider),
    Exa(ExaSearchProvider),
    Tavily(TavilySearchProvider),
    Firecrawl(FirecrawlSearchProvider),
}

impl std::fmt::Debug for SearchProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SearchProvider::{}", self.name())
    }
}

impl SearchProvider {
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        match self {
            SearchProvider::DuckDuckGo(p) => p.search(query, limit).await,
            SearchProvider::Exa(p) => p.search(query, limit).await,
            SearchProvider::Tavily(p) => p.search(query, limit).await,
            SearchProvider::Firecrawl(p) => p.search(query, limit).await,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            SearchProvider::DuckDuckGo(_) => "duckduckgo",
            SearchProvider::Exa(_) => "exa",
            SearchProvider::Tavily(_) => "tavily",
            SearchProvider::Firecrawl(_) => "firecrawl",
        }
    }
}

// ============================================================================
// DuckDuckGo provider — 免费，无需 API Key
// ============================================================================

#[derive(Clone)]
struct DuckDuckGoProvider {
    client: Client,
}

impl DuckDuckGoProvider {
    fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client builder"),
        }
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = self.client.get(&url).send().await
            .map_err(|e| ToolError::Execution(format!("DuckDuckGo error: {}", e)))?;
        let body = response.text().await
            .map_err(|e| ToolError::Execution(format!("DuckDuckGo error: {}", e)))?;

        let results = self.parse_html(&body, limit);
        Ok(results)
    }

    fn parse_html(&self, html: &str, num_results: usize) -> Vec<SearchResult> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let result_selector = Selector::parse("a.result__a").unwrap();

        let mut results = Vec::new();
        for (idx, element) in document.select(&result_selector).enumerate() {
            if idx >= num_results {
                break;
            }
            if let Some(href) = element.value().attr("href") {
                let title = element.text().collect::<String>();
                results.push(SearchResult {
                    url: href.to_string(),
                    title: title.trim().to_string(),
                    snippet: String::new(),
                    content: None,
                });
            }
        }
        results
    }
}

impl Default for DuckDuckGoProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ExaSearchProvider — Exa AI (exa.ai)
// ============================================================================

#[derive(Clone)]
pub struct ExaSearchProvider {
    api_key: String,
    client: Client,
}

impl ExaSearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let payload = serde_json::json!({
            "query": query,
            "num_results": limit,
            "contents": ["html"]
        });

        let resp = self.client
            .post("https://api.exa.ai/search")
            .header("x-api-key", &self.api_key)
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Exa API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Exa response: {}", e)))?;

        let results = body["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            url: r["url"].as_str()?.to_string(),
                            title: r["title"].as_str().unwrap_or("").to_string(),
                            snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                            content: r["html"].as_str().map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

// ============================================================================
// TavilySearchProvider — Tavily AI (tavily.ai)
// ============================================================================

#[derive(Clone)]
pub struct TavilySearchProvider {
    api_key: String,
    client: Client,
}

impl TavilySearchProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let resp = self.client
            .get("https://api.tavily.com/search")
            .query(&[
                ("query", query),
                ("api_key", &self.api_key),
                ("max_results", &limit.to_string()),
            ])
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Tavily API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Tavily response: {}", e)))?;

        let results = body["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            url: r["url"].as_str()?.to_string(),
                            title: r["title"].as_str().unwrap_or("").to_string(),
                            snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                            content: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

// ============================================================================
// FirecrawlSearchProvider — Firecrawl (firecrawl.dev)
// ============================================================================

#[derive(Clone)]
pub struct FirecrawlSearchProvider {
    api_key: String,
    engine: String,
    client: Client,
}

impl FirecrawlSearchProvider {
    pub fn new(api_key: String, engine: &str) -> Self {
        Self {
            api_key,
            engine: engine.to_string(),
            client: Client::new(),
        }
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, ToolError> {
        let payload = serde_json::json!({
            "query": query,
            "limit": limit,
            "engine": self.engine
        });

        let resp = self.client
            .post("https://api.firecrawl.dev/v0/search")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Firecrawl API error: {}", e)))?;

        let body: serde_json::Value = resp.json().await
            .map_err(|e| ToolError::Execution(format!("Invalid Firecrawl response: {}", e)))?;

        let results = body["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(SearchResult {
                            url: r["url"].as_str()?.to_string(),
                            title: r["title"].as_str().unwrap_or("").to_string(),
                            snippet: r["description"].as_str().unwrap_or("").to_string(),
                            content: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

// ============================================================================
// WebSearchTool
// ============================================================================

/// WebSearchTool — 网页搜索工具，支持多 provider
#[derive(Clone)]
pub struct WebSearchTool {
    #[allow(dead_code)]
    pub providers: HashMap<String, SearchProvider>,
}

impl std::fmt::Debug for WebSearchTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSearchTool")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl WebSearchTool {
    pub fn new() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "duckduckgo".to_string(),
            SearchProvider::DuckDuckGo(DuckDuckGoProvider::new()),
        );
        Self { providers }
    }

    pub fn with_exa(mut self, api_key: String) -> Self {
        self.providers.insert(
            "exa".to_string(),
            SearchProvider::Exa(ExaSearchProvider::new(api_key)),
        );
        self
    }

    pub fn with_tavily(mut self, api_key: String) -> Self {
        self.providers.insert(
            "tavily".to_string(),
            SearchProvider::Tavily(TavilySearchProvider::new(api_key)),
        );
        self
    }

    pub fn with_firecrawl(mut self, api_key: String, engine: &str) -> Self {
        self.providers.insert(
            "firecrawl".to_string(),
            SearchProvider::Firecrawl(FirecrawlSearchProvider::new(api_key, engine)),
        );
        self
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web. Supports: duckduckgo (free, no API key), exa, tavily, firecrawl (API keys required)."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return",
                    "default": 5
                },
                "provider": {
                    "type": "string",
                    "description": "Search provider to use",
                    "enum": ["duckduckgo", "exa", "tavily", "firecrawl"],
                    "default": "duckduckgo"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("query is required".to_string()))?;

        let num_results = args["num_results"]
            .as_u64()
            .unwrap_or(5) as usize;

        let provider_name = args["provider"].as_str().unwrap_or("duckduckgo");

        let provider = self.providers.get(provider_name)
            .ok_or_else(|| ToolError::InvalidArgs(
                format!(
                    "Unknown provider: {}. Available: {}",
                    provider_name,
                    self.providers.keys().cloned().collect::<Vec<_>>().join(", ")
                )
            ))?;

        let results = provider.search(query, num_results).await?;

        Ok(json!({
            "success": true,
            "query": query,
            "results": results,
            "provider": provider_name
        }).to_string())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_returns_results() {
        let tool = WebSearchTool::new();
        let result = tool.search("rust programming", 3, "duckduckgo").await;
        // Network-dependent test - may fail if DuckDuckGo is unavailable
        if let Ok(json_str) = result {
            let json: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[tokio::test]
    async fn test_name_and_description() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            snippet: "An example page".to_string(),
            content: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("example.com"));
    }
}

// Helper for internal tests (not part of Tool trait)
impl WebSearchTool {
    async fn search(&self, query: &str, num_results: usize, provider: &str) -> Result<String, ToolError> {
        let provider = self.providers.get(provider)
            .ok_or_else(|| ToolError::InvalidArgs(format!("Unknown provider: {}", provider)))?;
        let results = provider.search(query, num_results).await?;
        Ok(serde_json::to_string_pretty(&results).unwrap())
    }
}
