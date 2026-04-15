//! WebFetchTool — 网页内容抓取工具
//!
//! 抓取 URL 内容并可选择使用正则提取特定内容。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use reqwest::Client;
use serde_json::json;
use regex::Regex;

/// WebFetchTool — 网页内容抓取工具
#[derive(Debug, Clone)]
pub struct WebFetchTool {
    client: Client,
}

impl WebFetchTool {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client builder"),
        }
    }

    /// 抓取网页内容
    pub async fn fetch(&self, url: &str, extract_pattern: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let body = response.text().await?;

        // 如果有提取模式，应用正则
        if let Some(pattern) = extract_pattern {
            if let Ok(re) = Regex::new(pattern) {
                let matches: Vec<&str> = re.find_iter(&body)
                    .map(|m| m.as_str())
                    .collect();
                if !matches.is_empty() {
                    return Ok(matches.join("\n"));
                }
            }
        }

        // 清理 HTML 标签，提取纯文本
        let text = self.extract_text(&body);
        Ok(text)
    }

    fn extract_text(&self, html: &str) -> String {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        // 移除 script 和 style 标签
        let text_selector = Selector::parse("body").unwrap();

        let mut text = String::new();
        for element in document.select(&text_selector) {
            for line in element.text() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    text.push_str(trimmed);
                    text.push(' ');
                }
            }
        }

        text.trim().to_string()
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch and extract content from a URL. Optionally apply a regex pattern to extract specific content."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "extract_pattern": {
                    "type": "string",
                    "description": "Optional regex pattern to extract specific content"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("url is required".to_string()))?
            .to_string();

        let extract_pattern = args["extract_pattern"].as_str().filter(|s| !s.is_empty());

        self.fetch(&url, extract_pattern).await
            .map_err(|e| ToolError::Execution(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_name_and_description() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "web_fetch");
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn test_fetch_url() {
        let tool = WebFetchTool::new();
        // Test fetching a simple page
        let result = tool.fetch("https://example.com", None).await;
        assert!(result.is_ok());
        let text = result.unwrap();
        assert!(!text.is_empty());
        assert!(text.contains("Example"));
    }
}