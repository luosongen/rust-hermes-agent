//! hermes-environment — 终端后端抽象层
//!
//! 本 crate 提供统一的 `Environment` trait，允许 AI Agent 在不同执行后端上运行命令：
//! - **Local** — 本地进程（默认）
//! - **Docker** — Docker 容器内执行
//! - **SSH** — 通过 SSH 在远程主机执行
//!
//! ## 设计目标
//! - 统一接口：无论底层是本地、Docker 还是 SSH，工具调用方无感知
//! - 可配置：通过配置文件或环境变量切换后端
//! - 可扩展：新增后端只需实现 `Environment` trait
//!
//! ## 主要类型
//! - **`Environment`**（trait）— 后端统一接口
//! - **`LocalEnvironment`** — 本地进程执行
//! - **`DockerEnvironment`** — Docker 容器执行
//! - **`SSHEnvironment`** — SSH 远程执行
//! - **`EnvironmentManager`** — 环境配置解析与工厂
//! - **`ExecutionResult`** — 命令执行结果
//! - **`EnvironmentError`** — 环境相关错误

pub mod error;
pub mod local;
pub mod docker;
pub mod ssh;
pub mod manager;

pub use error::{EnvironmentError, ExecutionResult};
pub use local::LocalEnvironment;
pub use docker::DockerEnvironment;
pub use ssh::SSHEnvironment;
pub use manager::{EnvironmentConfig, EnvironmentManager, EnvironmentType, DockerConfigSerde, SSHConfigSerde};

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// 统一的执行环境抽象接口
///
/// 所有终端后端（本地、Docker、SSH 等）必须实现此 trait，
/// 为工具层提供无感知的命令执行、文件读写能力。
#[async_trait]
pub trait Environment: Send + Sync {
    /// 环境名称标识
    fn name(&self) -> &str;

    /// 人类可读的环境描述
    fn description(&self) -> String;

    /// 环境类型
    fn environment_type(&self) -> EnvironmentType;

    /// 在工作目录下执行命令
    ///
    /// # 参数
    /// - `command` — 主命令（如 `ls`、`docker`、`ssh`）
    /// - `args` — 命令参数列表
    /// - `cwd` — 可选的工作目录（相对于环境的工作目录）
    /// - `timeout` — 可选的超时时间
    /// - `env_vars` — 可选的额外环境变量
    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout: Option<Duration>,
        env_vars: Option<&HashMap<String, String>>,
    ) -> Result<ExecutionResult, EnvironmentError>;

    /// 读取文件内容
    async fn read_file(&self, path: &Path) -> Result<String, EnvironmentError>;

    /// 写入文件内容
    async fn write_file(&self, path: &Path, content: &str) -> Result<(), EnvironmentError>;

    /// 检查路径是否存在
    async fn exists(&self, path: &Path) -> Result<bool, EnvironmentError>;

    /// 列出目录内容
    async fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, EnvironmentError>;

    /// 查找命令路径（类似 `which`）
    async fn which(&self, command: &str) -> Result<Option<String>, EnvironmentError>;
}

/// 目录条目
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_file: bool,
    pub is_dir: bool,
}
