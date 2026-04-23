use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Modal backend configuration for cloud GPU access
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModalBackend {
    #[serde(default)]
    pub enabled: bool,
    pub app_name: Option<String>,
    pub token_id: Option<String>,
    pub token_secret: Option<String>,
    pub working_directory: Option<PathBuf>,
}
