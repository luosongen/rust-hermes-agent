use serde::{Deserialize, Serialize};

/// Local backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalBackend {
    #[serde(default)]
    pub enabled: bool,
}
