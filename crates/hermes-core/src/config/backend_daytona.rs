use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Daytona backend configuration for cloud dev environments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaytonaBackend {
    #[serde(default)]
    pub enabled: bool,
    pub server_url: Option<String>,
    pub api_key: Option<String>,
    pub workspace_dir: Option<PathBuf>,
}
