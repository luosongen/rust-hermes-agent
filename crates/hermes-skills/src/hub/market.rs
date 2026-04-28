//! 市场客户端模块
//!
//! 提供与技能市场 API 交互的能力

use reqwest::Client;
use crate::hub::error::HubError;
use crate::hub::types::{MarketCategoriesResponse, MarketCategory, MarketSkill};

/// 市场客户端
///
/// 用于从技能市场获取分类和技能信息
#[derive(Clone)]
pub struct MarketClient {
    client: Client,
    base_url: String,
}

impl MarketClient {
    /// 创建市场客户端
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    /// 获取市场分类列表
    pub async fn fetch_categories(&self) -> Result<MarketCategoriesResponse, HubError> {
        let url = format!("{}/v1/skills", self.base_url);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(HubError::MarketApiError(format!(
                "HTTP {}", response.status()
            )));
        }
        let body = response.text().await?;
        let data: MarketCategoriesResponse = serde_json::from_str(&body)
            .map_err(|e| HubError::ParseError(e.to_string()))?;
        Ok(data)
    }

    /// 获取指定技能信息
    pub async fn fetch_skill(&self, category: &str, name: &str) -> Result<MarketSkill, HubError> {
        let url = format!("{}/v1/skills/{}/{}", self.base_url, category, name);
        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(HubError::MarketApiError(format!(
                "HTTP {}", response.status()
            )));
        }
        let body = response.text().await?;
        let skill: MarketSkill = serde_json::from_str(&body)
            .map_err(|e| HubError::ParseError(e.to_string()))?;
        Ok(skill)
    }

    /// 下载技能内容
    pub async fn download_skill(&self, download_url: &str) -> Result<String, HubError> {
        let response = self.client.get(download_url).send().await?;
        if !response.status().is_success() {
            return Err(HubError::DownloadFailed(format!(
                "HTTP {}", response.status()
            )));
        }
        let body = response.text().await?;
        Ok(body)
    }
}