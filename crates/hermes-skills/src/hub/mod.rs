//! Hub 模块 - 技能市场和管理
//!
//! 提供技能市场的浏览、搜索、同步和安装功能

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

/// Hub 客户端
///
/// 整合技能索引、市场客户端、浏览、同步和安装功能
pub struct HubClient {
    /// 技能索引
    pub index: SkillIndex,
    /// 市场客户端
    pub market: MarketClient,
    /// 浏览功能
    pub browse: Browse,
    /// 同步功能
    pub sync: Sync,
    /// 安装功能
    pub installer: Installer,
    /// Hub 配置
    pub config: HubConfig,
}

impl HubClient {
    /// 创建 Hub 客户端
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

    /// 获取技能目录路径
    pub fn skills_dir(&self) -> PathBuf {
        self.installer.skills_dir.clone()
    }
}