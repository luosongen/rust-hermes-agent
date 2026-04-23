use serde::{Deserialize, Serialize};
use crate::credentials::Secret;

/// Custom provider for user-defined providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: Secret<String>,
}
