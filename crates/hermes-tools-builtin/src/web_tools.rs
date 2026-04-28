//! Web Search Tool — 网络搜索工具
//!
//! 支持多种搜索服务：
//! - Tavily — AI 优化的搜索 API
//! - Exa — 神经网络搜索
//! - Firecrawl — 网页抓取和搜索
//!
//! ## 配置
//! 在配置文件中添加：
//! ```toml
//! [web_search]
//! provider = "tavily"
//! api_key = "tvly-xxx"
//! max_results = 5
//! ```

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use reqwest::Client;
use serde::{Deserialize, Serialize};

// =============================================================================
// Configuration
// =============================================================================

/// 搜索服务提供商
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchProvider {
    Tavily,
    Exa,
    Firecrawl,
}

impl Default for SearchProvider {
    fn default() -> Self {
        Self::Tavily
    }
}

/// Web Search 配置
#[derive(Clone, Serialize, Deserialize)]
pub struct WebSearchConfig {
    /// 搜索服务提供商
    #[serde(default)]
    pub provider: SearchProvider,

    /// API Key
    pub api_key: String,

    /// 最大结果数
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// 是否包含网页内容
    #[serde(default = "default_include_content")]
    pub include_content: bool,

    /// 请求超时（秒）
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

// 自定义 Debug 实现以隐藏 API Key
impl std::fmt::Debug for WebSearchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSearchConfig")
            .field("provider", &self.provider)
            .field("api_key", &"[REDACTED]")
            .field("max_results", &self.max_results)
            .field("include_content", &self.include_content)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

fn default_max_results() -> usize {
    5
}

fn default_include_content() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            provider: SearchProvider::default(),
            api_key: String::new(),
            max_results: default_max_results(),
            include_content: default_include_content(),
            timeout_seconds: default_timeout(),
        }
    }
}

// =============================================================================
// Search Results
// =============================================================================

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// 标题
    pub title: String,
    /// URL
    pub url: String,
    /// 摘要
    pub snippet: String,
    /// 完整内容（可选）
    #[serde(default)]
    pub content: Option<String>,
    /// 发布日期（可选）
    #[serde(default)]
    pub published_date: Option<String>,
    /// 来源
    #[serde(default)]
    pub source: Option<String>,
}

/// 搜索响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    /// 搜索结果列表
    pub results: Vec<SearchResult>,
    /// 查询词
    pub query: String,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
}

// =============================================================================
// Tavily API
// =============================================================================

/// Tavily 搜索请求
#[derive(Serialize)]
struct TavilyRequest {
    api_key: String,
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_results: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_raw_content: Option<bool>,
    search_depth: String,
}

/// Tavily 搜索响应
#[derive(Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: String,
    #[serde(default)]
    raw_content: Option<String>,
}

// =============================================================================
// Exa API
// =============================================================================

/// Exa 搜索请求
#[derive(Serialize)]
struct ExaRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_results: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    use_autoprompt: Option<bool>,
    contents: ExaContents,
}

#[derive(Serialize)]
struct ExaContents {
    text: ExaTextConfig,
}

#[derive(Serialize)]
struct ExaTextConfig {
    max_characters: usize,
}

/// Exa 搜索响应
#[derive(Deserialize)]
struct ExaResponse {
    results: Vec<ExaResult>,
}

#[derive(Deserialize)]
struct ExaResult {
    title: String,
    url: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    published_date: Option<String>,
    #[serde(default)]
    author: Option<String>,
}

// =============================================================================
// Firecrawl API
// =============================================================================

/// Firecrawl 搜索请求
#[derive(Serialize)]
struct FirecrawlSearchRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<usize>,
}

/// Firecrawl 搜索响应
#[derive(Deserialize)]
struct FirecrawlSearchResponse {
    data: Vec<FirecrawlSearchResult>,
}

#[derive(Deserialize)]
struct FirecrawlSearchResult {
    markdown: String,
    metadata: FirecrawlMetadata,
}

#[derive(Deserialize)]
struct FirecrawlMetadata {
    title: Option<String>,
    source_url: Option<String>,
    #[serde(default)]
    published_time: Option<String>,
}

// =============================================================================
// Web Search Tool
// =============================================================================

/// Web Search 工具
pub struct WebSearchTool {
    config: WebSearchConfig,
    client: Client,
}

impl WebSearchTool {
    /// 创建新的 Web Search 工具
    pub fn new(config: WebSearchConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_seconds))
            .user_agent("hermes-agent/1.0")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { config, client }
    }

    /// 使用 Tavily 搜索
    async fn search_tavily(&self, query: &str) -> Result<Vec<SearchResult>, ToolError> {
        let request = TavilyRequest {
            api_key: self.config.api_key.clone(),
            query: query.to_string(),
            max_results: Some(self.config.max_results),
            include_raw_content: if self.config.include_content { Some(true) } else { None },
            search_depth: "basic".to_string(),
        };

        let response = self
            .client
            .post("https://api.tavily.com/search")
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Tavily API 错误: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!(
                "Tavily API 失败: {} - {}",
                status, body
            )));
        }

        let tavily_response: TavilyResponse = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("解析响应失败: {}", e)))?;

        Ok(tavily_response
            .results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.content.clone(),
                content: r.raw_content.or(Some(r.content)),
                published_date: None,
                source: Some("tavily".to_string()),
            })
            .collect())
    }

    /// 使用 Exa 搜索
    async fn search_exa(&self, query: &str) -> Result<Vec<SearchResult>, ToolError> {
        let request = ExaRequest {
            query: query.to_string(),
            num_results: Some(self.config.max_results),
            use_autoprompt: Some(true),
            contents: ExaContents {
                text: ExaTextConfig {
                    max_characters: if self.config.include_content { 4000 } else { 500 },
                },
            },
        };

        let response = self
            .client
            .post("https://api.exa.ai/search")
            .header("x-api-key", &self.config.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Exa API 错误: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!(
                "Exa API 失败: {} - {}",
                status, body
            )));
        }

        let exa_response: ExaResponse = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("解析响应失败: {}", e)))?;

        Ok(exa_response
            .results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.text.clone().unwrap_or_default().chars().take(300).collect(),
                content: r.text,
                published_date: r.published_date,
                source: r.author.or(Some("exa".to_string())),
            })
            .collect())
    }

    /// 使用 Firecrawl 搜索
    async fn search_firecrawl(&self, query: &str) -> Result<Vec<SearchResult>, ToolError> {
        let request = FirecrawlSearchRequest {
            query: query.to_string(),
            limit: Some(self.config.max_results),
        };

        let response = self
            .client
            .post("https://api.firecrawl.dev/v1/search")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Firecrawl API 错误: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ToolError::Execution(format!(
                "Firecrawl API 失败: {} - {}",
                status, body
            )));
        }

        let fc_response: FirecrawlSearchResponse = response
            .json()
            .await
            .map_err(|e| ToolError::Execution(format!("解析响应失败: {}", e)))?;

        Ok(fc_response
            .data
            .into_iter()
            .map(|r| SearchResult {
                title: r.metadata.title.unwrap_or_else(|| "无标题".to_string()),
                url: r.metadata.source_url.unwrap_or_default(),
                snippet: r.markdown.chars().take(300).collect(),
                content: if self.config.include_content {
                    Some(r.markdown)
                } else {
                    None
                },
                published_date: r.metadata.published_time,
                source: Some("firecrawl".to_string()),
            })
            .collect())
    }

    /// 执行搜索
    async fn search(&self, query: &str) -> Result<SearchResponse, ToolError> {
        let start = std::time::Instant::now();

        let results = match self.config.provider {
            SearchProvider::Tavily => self.search_tavily(query).await?,
            SearchProvider::Exa => self.search_exa(query).await?,
            SearchProvider::Firecrawl => self.search_firecrawl(query).await?,
        };

        Ok(SearchResponse {
            results,
            query: query.to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 格式化搜索结果为 Markdown
    fn format_results(&self, response: &SearchResponse) -> String {
        let mut output = format!("## 搜索结果: {}\n\n", response.query);
        output.push_str(&format!("找到 {} 条结果 (耗时 {}ms)\n\n", response.results.len(), response.duration_ms));

        for (i, result) in response.results.iter().enumerate() {
            output.push_str(&format!("### {}. {}\n", i + 1, result.title));
            output.push_str(&format!("**URL**: {}\n\n", result.url));

            if let Some(date) = &result.published_date {
                output.push_str(&format!("**发布日期**: {}\n\n", date));
            }

            output.push_str(&format!("**摘要**: {}\n\n", result.snippet));

            if let Some(content) = &result.content {
                if self.config.include_content {
                    output.push_str(&format!("**内容**:\n```\n{}\n```\n\n", content));
                }
            }

            output.push_str("---\n\n");
        }

        output
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "搜索互联网获取最新信息。支持多种搜索服务提供商。"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索查询"
                },
                "max_results": {
                    "type": "integer",
                    "description": "最大结果数（默认 5）",
                    "default": 5
                },
                "include_content": {
                    "type": "boolean",
                    "description": "是否包含完整网页内容",
                    "default": true
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
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("缺少 query 参数".to_string()))?;

        // 允许覆盖配置
        let max_results = args
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(self.config.max_results);

        let include_content = args
            .get("include_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(self.config.include_content);

        // 临时修改配置
        let mut config = self.config.clone();
        config.max_results = max_results;
        config.include_content = include_content;

        let response = self.search(query).await?;
        Ok(self.format_results(&response))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = WebSearchConfig::default();
        assert_eq!(config.max_results, 5);
        assert!(config.include_content);
        assert_eq!(config.timeout_seconds, 30);
    }

    #[test]
    fn test_format_results() {
        let tool = WebSearchTool::new(WebSearchConfig {
            provider: SearchProvider::Tavily,
            api_key: "test".to_string(),
            max_results: 5,
            include_content: false,
            timeout_seconds: 30,
        });

        let response = SearchResponse {
            results: vec![SearchResult {
                title: "Test Result".to_string(),
                url: "https://example.com".to_string(),
                snippet: "This is a test snippet.".to_string(),
                content: None,
                published_date: Some("2024-01-01".to_string()),
                source: Some("tavily".to_string()),
            }],
            query: "test query".to_string(),
            duration_ms: 100,
        };

        let output = tool.format_results(&response);
        assert!(output.contains("Test Result"));
        assert!(output.contains("https://example.com"));
        assert!(output.contains("test snippet"));
    }
}
