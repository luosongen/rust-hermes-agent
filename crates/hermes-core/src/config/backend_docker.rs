use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Docker backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerBackend {
    #[serde(default)]
    pub enabled: bool,
    pub container: Option<String>,
    pub docker_host: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub auto_start: bool,
    pub user: Option<String>,
}
