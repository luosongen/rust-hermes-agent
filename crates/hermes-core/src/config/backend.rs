use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod backend_local;
mod backend_docker;
mod backend_ssh;
mod backend_singularity;
mod backend_modal;
mod backend_daytona;

pub use backend_local::LocalBackend;
pub use backend_docker::DockerBackend;
pub use backend_ssh::SSHBackend;
pub use backend_singularity::SingularityBackend;
pub use backend_modal::ModalBackend;
pub use backend_daytona::DaytonaBackend;

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
