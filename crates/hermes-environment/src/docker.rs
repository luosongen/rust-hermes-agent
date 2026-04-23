//! DockerEnvironment — Docker 容器执行后端
//!
//! 在指定的 Docker 容器内执行命令，支持本地和远程 Docker daemon。

use crate::{DirEntry, Environment, EnvironmentError, ExecutionResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Docker 执行环境配置
#[derive(Debug, Clone)]
pub struct DockerConfig {
    /// 容器名称或 ID
    pub container: String,
    /// 可选的 Docker 主机（如 `tcp://remote:2375`）
    pub docker_host: Option<String>,
    /// 容器内的工作目录
    pub working_directory: PathBuf,
    /// 是否在执行前自动启动容器
    pub auto_start: bool,
    /// 执行命令的用户（容器内）
    pub user: Option<String>,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            container: String::new(),
            docker_host: None,
            working_directory: PathBuf::from("/workspace"),
            auto_start: false,
            user: None,
        }
    }
}

/// Docker 容器执行环境
///
/// 通过 `docker exec` 在指定容器内执行命令。
#[derive(Debug, Clone)]
pub struct DockerEnvironment {
    config: DockerConfig,
}

impl DockerEnvironment {
    /// 创建新的 Docker 环境
    pub fn new(config: DockerConfig) -> Self {
        Self { config }
    }

    /// 获取配置引用
    pub fn config(&self) -> &DockerConfig {
        &self.config
    }

    /// 构建 docker exec 命令的基础参数
    fn build_base_cmd(&self) -> Command {
        let mut cmd = Command::new("docker");

        if let Some(ref host) = self.config.docker_host {
            cmd.env("DOCKER_HOST", host);
        }

        cmd.arg("exec");
        cmd.arg("-i"); // 交互式（保持 stdin 打开）

        if let Some(ref user) = self.config.user {
            cmd.arg("--user").arg(user);
        }

        cmd.arg("--workdir").arg(&self.config.working_directory);
        cmd.arg(&self.config.container);

        cmd
    }

    /// 检查容器是否正在运行
    async fn ensure_container_running(&self) -> Result<(), EnvironmentError> {
        let mut cmd = Command::new("docker");
        if let Some(ref host) = self.config.docker_host {
            cmd.env("DOCKER_HOST", host);
        }

        let inspect = cmd
            .arg("inspect")
            .arg("--format={{.State.Status}}")
            .arg(&self.config.container)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| EnvironmentError::Connection(format!("Failed to run docker inspect: {}", e)))?;

        let status = String::from_utf8_lossy(&inspect.stdout).trim().to_string();

        if status == "running" {
            return Ok(());
        }

        if self.config.auto_start && (status == "exited" || status == "created") {
            tracing::info!("[DockerEnv] Starting container {}", self.config.container);
            let mut start_cmd = Command::new("docker");
            if let Some(ref host) = self.config.docker_host {
                start_cmd.env("DOCKER_HOST", host);
            }
            let start = start_cmd
                .arg("start")
                .arg(&self.config.container)
                .output()
                .await
                .map_err(|e| EnvironmentError::Execution(format!("Failed to start container: {}", e)))?;

            if !start.status.success() {
                let err = String::from_utf8_lossy(&start.stderr);
                return Err(EnvironmentError::Execution(format!("Failed to start container: {}", err)));
            }

            // 等待容器就绪
            tokio::time::sleep(Duration::from_millis(500)).await;
            return Ok(());
        }

        Err(EnvironmentError::Connection(format!(
            "Container {} is not running (status: {})",
            self.config.container, status
        )))
    }
}

#[async_trait]
impl Environment for DockerEnvironment {
    fn name(&self) -> &str {
        "docker"
    }

    fn description(&self) -> String {
        format!("Execute commands inside Docker container: {}", self.config.container)
    }

    fn environment_type(&self) -> crate::manager::EnvironmentType {
        crate::manager::EnvironmentType::Docker
    }

    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout_dur: Option<Duration>,
        _env_vars: Option<&HashMap<String, String>>,
    ) -> Result<ExecutionResult, EnvironmentError> {
        self.ensure_container_running().await?;

        let mut cmd = self.build_base_cmd();

        // 覆盖工作目录（如果指定）
        if let Some(cwd_path) = cwd {
            // Docker exec 不支持动态修改 workdir，需要使用 sh -c "cd ... && cmd"
            let cd_path = cwd_path.to_string_lossy();
            let cmd_str = format!("cd {} && {} {}", cd_path, command, args.join(" "));
            cmd = self.build_base_cmd();
            cmd.arg("sh").arg("-c").arg(&cmd_str);
        } else {
            cmd.arg(command).args(args);
        }

        let cmd_str = format!("docker exec {} {} {}", self.config.container, command, args.join(" "));
        tracing::debug!("[DockerEnv] Executing: {}", cmd_str);

        let output = if let Some(dur) = timeout_dur {
            timeout(dur, cmd.output())
                .await
                .map_err(|_| EnvironmentError::Timeout(format!("Docker exec timed out after {:?}", dur)))?
                .map_err(|e| EnvironmentError::Execution(format!("Docker exec failed: {}", e)))?
        } else {
            cmd.output()
                .await
                .map_err(|e| EnvironmentError::Execution(format!("Docker exec failed: {}", e)))?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ExecutionResult::new(cmd_str, output.status.code(), stdout, stderr))
    }

    async fn read_file(&self, path: &Path) -> Result<String, EnvironmentError> {
        self.ensure_container_running().await?;

        let container_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        let mut cmd = self.build_base_cmd();
        cmd.arg("cat").arg(&container_path);

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Execution(format!("Failed to read file: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("No such file") || err.contains("not found") {
                return Err(EnvironmentError::PathNotFound(container_path.display().to_string()));
            }
            return Err(EnvironmentError::Execution(format!("Failed to read file: {}", err)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<(), EnvironmentError> {
        self.ensure_container_running().await?;

        let container_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        // 使用 docker exec sh -c "cat > file" 写入
        // 为了避免 shell 转义问题，使用 base64 编码
        let encoded = base64::encode(content.as_bytes());
        let write_cmd = format!(
            "mkdir -p $(dirname {}) && echo {} | base64 -d > {}",
            container_path.display(),
            encoded,
            container_path.display()
        );

        let mut cmd = self.build_base_cmd();
        cmd.arg("sh").arg("-c").arg(&write_cmd);

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Execution(format!("Failed to write file: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(EnvironmentError::Execution(format!("Failed to write file: {}", err)));
        }

        Ok(())
    }

    async fn exists(&self, path: &Path) -> Result<bool, EnvironmentError> {
        self.ensure_container_running().await?;

        let container_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        let mut cmd = self.build_base_cmd();
        cmd.arg("test").arg("-e").arg(&container_path);

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Execution(format!("Failed to check existence: {}", e)))?;

        Ok(output.status.success())
    }

    async fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, EnvironmentError> {
        self.ensure_container_running().await?;

        let container_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        let mut cmd = self.build_base_cmd();
        cmd.arg("ls").arg("-1").arg(&container_path);

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Execution(format!("Failed to list directory: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("No such file") || err.contains("not found") {
                return Err(EnvironmentError::PathNotFound(container_path.display().to_string()));
            }
            return Err(EnvironmentError::Execution(format!("Failed to list directory: {}", err)));
        }

        // 使用 stat 获取文件类型信息
        let mut entries = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        for name in stdout.lines() {
            if name.is_empty() {
                continue;
            }
            let entry_path = container_path.join(name);
            let mut stat_cmd = self.build_base_cmd();
            stat_cmd.arg("stat").arg("-c").arg("%F").arg(&entry_path);

            let stat_out = stat_cmd
                .output()
                .await
                .map_err(|e| EnvironmentError::Execution(format!("Failed to stat: {}", e)))?;

            let file_type = String::from_utf8_lossy(&stat_out.stdout).trim().to_string();
            let is_dir = file_type.contains("directory");
            let is_file = file_type.contains("regular file");

            entries.push(DirEntry {
                name: name.to_string(),
                is_file,
                is_dir,
            });
        }

        Ok(entries)
    }

    async fn which(&self, command: &str) -> Result<Option<String>, EnvironmentError> {
        self.ensure_container_running().await?;

        let mut cmd = self.build_base_cmd();
        cmd.arg("which").arg(command);

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Execution(format!("Failed to run which: {}", e)))?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path))
            }
        } else {
            Ok(None)
        }
    }
}

// base64 encode helper (use a simple inline implementation to avoid extra deps)
mod base64 {
    pub fn encode(input: &[u8]) -> String {
        const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::new();
        for chunk in input.chunks(3) {
            let buf = match chunk.len() {
                1 => [chunk[0], 0, 0],
                2 => [chunk[0], chunk[1], 0],
                _ => [chunk[0], chunk[1], chunk[2]],
            };
            let b = [
                TABLE[(buf[0] >> 2) as usize],
                TABLE[(((buf[0] & 0x3) << 4) | (buf[1] >> 4)) as usize],
                if chunk.len() > 1 { TABLE[(((buf[1] & 0xf) << 2) | (buf[2] >> 6)) as usize] } else { b'=' },
                if chunk.len() > 2 { TABLE[(buf[2] & 0x3f) as usize] } else { b'=' },
            ];
            output.push_str(&String::from_utf8_lossy(&b));
        }
        output
    }
}
