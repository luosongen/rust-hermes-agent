use reqwest::Client;
use crate::hub::error::HubError;
use crate::hub::types::{MarketCategoriesResponse, MarketCategory, MarketSkill};

#[derive(Clone)]
pub struct MarketClient {
    client: Client,
    base_url: String,
}

impl MarketClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

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