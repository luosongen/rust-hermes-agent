pub mod browse;
pub mod error;
pub mod index;
pub mod installer;
pub mod market;
pub mod security;
pub mod sync;
pub mod types;

pub use browse::Browse;
pub use error::HubError;
pub use index::SkillIndex;
pub use installer::Installer;
pub use market::MarketClient;
pub use security::{SecurityScanner, SecurityScanResult, SecurityThreat, Severity, ThreatType};
pub use sync::Sync;
pub use types::*;

use std::path::PathBuf;

pub struct HubClient {
    pub index: SkillIndex,
    pub market: MarketClient,
    pub browse: Browse,
    pub sync: Sync,
    pub installer: Installer,
    pub config: HubConfig,
}

impl HubClient {
    pub fn new(home_dir: PathBuf) -> Result<Self, HubError> {
        let config = HubConfig::default();
        let skills_dir = home_dir.join("skills");
        let db_path = home_dir.join("skills_index.db");

        std::fs::create_dir_all(&skills_dir)?;

        let index = SkillIndex::new(db_path)?;
        let market = MarketClient::new(config.default_hub.clone());
        let browse = Browse::new(index.clone());
        let sync = Sync::new(index.clone(), market.clone(), config.clone());
        let installer = Installer::new(index.clone(), market.clone(), skills_dir.clone());

        Ok(Self {
            index,
            market,
            browse,
            sync,
            installer,
            config,
        })
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.installer.skills_dir.clone()
    }
}