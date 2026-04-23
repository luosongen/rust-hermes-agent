use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// Backend type definitions
// ============================================================================

/// Local backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalBackend {
    #[serde(default)]
    pub enabled: bool,
}

/// Docker backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerBackend {
    #[serde(default)]
    pub enabled: bool,
    pub image: Option<String>,
    pub container: Option<String>,
}

/// SSH backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SSHBackend {
    #[serde(default)]
    pub enabled: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
}

/// Singularity backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SingularityBackend {
    #[serde(default)]
    pub enabled: bool,
    pub image: Option<String>,
}

/// Modal backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModalBackend {
    #[serde(default)]
    pub enabled: bool,
}

/// Daytona backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaytonaBackend {
    #[serde(default)]
    pub enabled: bool,
}

// ============================================================================
// Backend configuration enum
// ============================================================================

/// Backend configuration enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BackendConfig {
    #[serde(rename = "local")]
    Local(LocalBackend),
    #[serde(rename = "docker")]
    Docker(DockerBackend),
    #[serde(rename = "ssh")]
    SSH(SSHBackend),
    #[serde(rename = "singularity")]
    Singularity(SingularityBackend),
    #[serde(rename = "modal")]
    Modal(ModalBackend),
    #[serde(rename = "daytona")]
    Daytona(DaytonaBackend),
}

impl Default for BackendConfig {
    fn default() -> Self {
        BackendConfig::Local(LocalBackend { enabled: true })
    }
}

/// Backend settings with default selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSettings {
    #[serde(default = "default_backend")]
    pub default: BackendConfig,
    #[serde(default)]
    pub workdir: PathBuf,
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            default: BackendConfig::default(),
            workdir: PathBuf::from("."),
        }
    }
}

fn default_backend() -> BackendConfig { BackendConfig::default() }
