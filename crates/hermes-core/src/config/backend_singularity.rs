use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Singularity backend configuration for HPC environments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SingularityBackend {
    #[serde(default)]
    pub enabled: bool,
    pub image: Option<String>,
    pub bind_paths: Option<Vec<String>>,
    pub working_directory: Option<PathBuf>,
}
