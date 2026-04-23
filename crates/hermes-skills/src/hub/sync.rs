use chrono::Utc;
use crate::hub::error::HubError;
use crate::hub::index::SkillIndex;
use crate::hub::market::MarketClient;
use crate::hub::types::{Category, HubConfig};

pub struct Sync {
    index: SkillIndex,
    market: MarketClient,
    config: HubConfig,
}

impl Sync {
    pub fn new(index: SkillIndex, market: MarketClient, config: HubConfig) -> Self {
        Self {
            index,
            market,
            config,
        }
    }

    pub async fn sync_categories(&self) -> Result<Vec<Category>, HubError> {
        // Fetch from market
        let response = self.market.fetch_categories().await?;

        // Update local categories
        for cat in &response.categories {
            self.index.add_category(&Category {
                name: cat.name.clone(),
                description: cat.description.clone(),
                icon: None,
                skill_count: cat.skills.len(),
            })?;
        }

        Ok(response.categories.into_iter().map(|c| Category {
            name: c.name,
            description: c.description,
            icon: None,
            skill_count: c.skills.len(),
        }).collect())
    }

    pub fn get_last_sync_time(&self) -> Result<Option<String>, HubError> {
        let conn = self.index.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT synced_at FROM sync_log ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get::<_, String>(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn record_sync(&self, hub_url: &str, skills_count: usize, status: &str) -> Result<(), HubError> {
        let conn = self.index.conn.lock();
        conn.execute(
            "INSERT INTO sync_log (hub_url, synced_at, skills_count, status) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                hub_url,
                Utc::now().to_rfc3339(),
                skills_count as i64,
                status,
            ],
        )?;
        Ok(())
    }
}