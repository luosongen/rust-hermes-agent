use crate::credential_pool::Secret;
use serde::{Deserialize, Serialize};

/// Custom provider for user-defined providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProviderConfig {
    pub name: String,
    pub base_url: String,
    pub api_key: Secret<String>,
}
