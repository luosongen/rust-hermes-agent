//! WebSearchTool — 网页搜索工具
//!
//! 使用 DuckDuckGo HTML API 进行免费网页搜索，无需 API Key。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use reqwest::Client;
use serde_json::json;
use std::error::Error;

/// WebSearchTool — 网页搜索工具
#[derive(Debug, Clone)]
pub struct WebSearchTool {
    client: Client,
}

impl WebSearchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client builder"),
        }
    }

    /// 执行 DuckDuckGo 搜索
    pub async fn search(&self, query: &str, num_results: usize) -> Result<String, Box<dyn Error>> {
        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;

        // 解析 DuckDuckGo HTML 结果
        let results = self.parse_ddg_html(&body, num_results);
        Ok(serde_json::to_string_pretty(&results)?)
    }

    fn parse_ddg_html(&self, html: &str, num_results: usize) -> Vec<serde_json::Value> {
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
                results.push(json!({
                    "title": title.trim(),
                    "url": href
                }));
            }
        }
        results
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
        "Search the web for information using DuckDuckGo. Returns a list of search results with titles and URLs."
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

        self.search(query, num_results).await
            .map_err(|e| ToolError::Execution(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_returns_results() {
        let tool = WebSearchTool::new();
        let result = tool.search("rust programming", 3).await;
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
}
