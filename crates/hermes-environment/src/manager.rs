//! EnvironmentManager — 环境配置解析与工厂
//!
//! 负责从配置文件或环境变量解析环境配置，并创建对应的 `Environment` 实例。

use crate::{Environment, LocalEnvironment};
use crate::docker::{DockerConfig, DockerEnvironment};
use crate::ssh::{SSHConfig, SSHEnvironment};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// 环境类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentType {
    Local,
    Docker,
    SSH,
    // 未来可扩展：Modal, Daytona, Singularity
}

impl Default for EnvironmentType {
    fn default() -> Self {
        EnvironmentType::Local
    }
}

impl std::fmt::Display for EnvironmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvironmentType::Local => write!(f, "local"),
            EnvironmentType::Docker => write!(f, "docker"),
            EnvironmentType::SSH => write!(f, "ssh"),
        }
    }
}

impl std::str::FromStr for EnvironmentType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(EnvironmentType::Local),
            "docker" => Ok(EnvironmentType::Docker),
            "ssh" => Ok(EnvironmentType::SSH),
            _ => Err(format!("Unknown environment type: {}", s)),
        }
    }
}

/// 环境配置（用于序列化/反序列化）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvironmentConfig {
    /// 环境类型
    #[serde(default)]
    pub env_type: EnvironmentType,
    /// 通用工作目录
    #[serde(default = "default_working_dir")]
    pub working_directory: PathBuf,
    /// Docker 专用配置
    #[serde(default)]
    pub docker: DockerConfigSerde,
    /// SSH 专用配置
    #[serde(default)]
    pub ssh: SSHConfigSerde,
    /// 额外环境变量
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
}

fn default_working_dir() -> PathBuf {
    PathBuf::from(".")
}

/// Docker 配置（序列化友好版本）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DockerConfigSerde {
    pub container: Option<String>,
    pub docker_host: Option<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub auto_start: bool,
    pub user: Option<String>,
}

/// SSH 配置（序列化友好版本）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SSHConfigSerde {
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

fn default_ssh_port() -> u16 {
    22
}

/// 环境管理器
///
/// 解析配置并创建 `Environment` 实例的工厂。
pub struct EnvironmentManager;

impl EnvironmentManager {
    /// 从配置创建环境实例
    pub fn create(config: &EnvironmentConfig) -> Result<Arc<dyn Environment>, crate::EnvironmentError> {
        match config.env_type {
            EnvironmentType::Local => {
                let env = LocalEnvironment::new(&config.working_directory);
                Ok(Arc::new(env))
            }
            EnvironmentType::Docker => {
                let container = config.docker.container.clone().ok_or_else(|| {
                    crate::EnvironmentError::InvalidConfig(
                        "Docker environment requires 'docker.container' config".to_string(),
                    )
                })?;

                let docker_config = DockerConfig {
                    container,
                    docker_host: config.docker.docker_host.clone(),
                    working_directory: config
                        .docker
                        .working_directory
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("/workspace")),
                    auto_start: config.docker.auto_start,
                    user: config.docker.user.clone(),
                };

                Ok(Arc::new(DockerEnvironment::new(docker_config)))
            }
            EnvironmentType::SSH => {
                let host = config.ssh.host.clone().ok_or_else(|| {
                    crate::EnvironmentError::InvalidConfig(
                        "SSH environment requires 'ssh.host' config".to_string(),
                    )
                })?;
                let user = config.ssh.user.clone().ok_or_else(|| {
                    crate::EnvironmentError::InvalidConfig(
                        "SSH environment requires 'ssh.user' config".to_string(),
                    )
                })?;

                let ssh_config = SSHConfig {
                    host,
                    port: config.ssh.port,
                    user,
                    private_key: config.ssh.private_key.clone(),
                    password: config.ssh.password.clone(),
                    working_directory: config
                        .ssh
                        .working_directory
                        .clone()
                        .unwrap_or_else(|| PathBuf::from(".")),
                    ssh_options: config.ssh.ssh_options.clone(),
                };

                Ok(Arc::new(SSHEnvironment::new(ssh_config)))
            }
        }
    }

    /// 从环境变量创建默认环境
    ///
    /// 读取 `HERMES_ENVIRONMENT_TYPE` 环境变量，回退到 Local。
    pub fn from_env() -> Arc<dyn Environment> {
        let env_type = std::env::var("HERMES_ENVIRONMENT_TYPE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(EnvironmentType::Local);

        let working_dir = std::env::var("HERMES_ENVIRONMENT_WORKDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let mut config = EnvironmentConfig {
            env_type,
            working_directory: working_dir,
            ..Default::default()
        };

        // Docker 环境变量
        if env_type == EnvironmentType::Docker {
            if let Ok(container) = std::env::var("HERMES_DOCKER_CONTAINER") {
                config.docker.container = Some(container);
            }
            if let Ok(host) = std::env::var("HERMES_DOCKER_HOST") {
                config.docker.docker_host = Some(host);
            }
            if let Ok(workdir) = std::env::var("HERMES_DOCKER_WORKDIR") {
                config.docker.working_directory = Some(PathBuf::from(workdir));
            }
        }

        // SSH 环境变量
        if env_type == EnvironmentType::SSH {
            if let Ok(host) = std::env::var("HERMES_SSH_HOST") {
                config.ssh.host = Some(host);
            }
            if let Ok(user) = std::env::var("HERMES_SSH_USER") {
                config.ssh.user = Some(user);
            }
            if let Ok(port) = std::env::var("HERMES_SSH_PORT") {
                if let Ok(p) = port.parse() {
                    config.ssh.port = p;
                }
            }
            if let Ok(key) = std::env::var("HERMES_SSH_PRIVATE_KEY") {
                config.ssh.private_key = Some(PathBuf::from(key));
            }
        }

        match Self::create(&config) {
            Ok(env) => env,
            Err(e) => {
                tracing::warn!("Failed to create environment from env, falling back to local: {}", e);
                Arc::new(LocalEnvironment::new(&config.working_directory))
            }
        }
    }

    /// 从配置文件的 environment 节创建环境
    pub fn from_config(config: &crate::EnvironmentConfig) -> Result<Arc<dyn Environment>, crate::EnvironmentError> {
        Self::create(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_type_from_str() {
        assert_eq!("local".parse::<EnvironmentType>().unwrap(), EnvironmentType::Local);
        assert_eq!("docker".parse::<EnvironmentType>().unwrap(), EnvironmentType::Docker);
        assert_eq!("ssh".parse::<EnvironmentType>().unwrap(), EnvironmentType::SSH);
        assert!("unknown".parse::<EnvironmentType>().is_err());
    }

    #[test]
    fn test_create_local() {
        let config = EnvironmentConfig {
            env_type: EnvironmentType::Local,
            working_directory: PathBuf::from("/tmp"),
            ..Default::default()
        };
        let env = EnvironmentManager::create(&config).unwrap();
        assert_eq!(env.name(), "local");
    }

    #[test]
    fn test_create_docker_missing_container() {
        let config = EnvironmentConfig {
            env_type: EnvironmentType::Docker,
            ..Default::default()
        };
        assert!(EnvironmentManager::create(&config).is_err());
    }

    #[test]
    fn test_create_docker_ok() {
        let config = EnvironmentConfig {
            env_type: EnvironmentType::Docker,
            docker: DockerConfigSerde {
                container: Some("my-container".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let env = EnvironmentManager::create(&config).unwrap();
        assert_eq!(env.name(), "docker");
    }

    #[test]
    fn test_create_ssh_missing_host() {
        let config = EnvironmentConfig {
            env_type: EnvironmentType::SSH,
            ..Default::default()
        };
        assert!(EnvironmentManager::create(&config).is_err());
    }
}
