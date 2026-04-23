use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// SSH backend configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SSHBackend {
    #[serde(default)]
    pub enabled: bool,
    pub host: Option<String>,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub user: Option<String>,
    pub private_key: Option<PathBuf>,
    pub password: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub ssh_options: Vec<String>,
}

fn default_ssh_port() -> u16 { 22 }
