//! SSHEnvironment — SSH 远程执行后端
//!
//! 通过 SSH 在远程主机上执行命令，支持密码和密钥认证。

use crate::{DirEntry, Environment, EnvironmentError, ExecutionResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// SSH 执行环境配置
#[derive(Debug, Clone)]
pub struct SSHConfig {
    /// 远程主机地址
    pub host: String,
    /// SSH 端口
    pub port: u16,
    /// 用户名
    pub user: String,
    /// 可选的私钥路径
    pub private_key: Option<PathBuf>,
    /// 可选的密码（优先使用密钥）
    pub password: Option<String>,
    /// 远程工作目录
    pub working_directory: PathBuf,
    /// SSH 额外选项
    pub ssh_options: Vec<String>,
}

impl Default for SSHConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            user: String::new(),
            private_key: None,
            password: None,
            working_directory: PathBuf::from("."),
            ssh_options: Vec::new(),
        }
    }
}

/// SSH 远程执行环境
///
/// 通过本地 `ssh` 命令在远程主机上执行命令。
#[derive(Debug, Clone)]
pub struct SSHEnvironment {
    config: SSHConfig,
}

impl SSHEnvironment {
    /// 创建新的 SSH 环境
    pub fn new(config: SSHConfig) -> Self {
        Self { config }
    }

    /// 获取配置引用
    pub fn config(&self) -> &SSHConfig {
        &self.config
    }

    /// 构建 SSH 命令参数列表
    fn build_ssh_args(&self) -> Vec<String> {
        let mut args = vec![
            "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
            "-o".to_string(), "BatchMode=no".to_string(),
            "-o".to_string(), "ConnectTimeout=10".to_string(),
            "-p".to_string(), self.config.port.to_string(),
        ];

        // 私钥
        if let Some(ref key) = self.config.private_key {
            args.push("-i".to_string());
            args.push(key.to_string_lossy().to_string());
        }

        // 额外选项
        for opt in &self.config.ssh_options {
            args.push(opt.clone());
        }

        // 目标地址
        let target = format!("{}@{}", self.config.user, self.config.host);
        args.push(target);

        args
    }

    /// 构建完整的 SSH Command（包含 sshpass 包装）
    fn build_command(&self, remote_cmd: Option<&str>) -> Command {
        let ssh_args = self.build_ssh_args();
        let use_sshpass = self.config.password.is_some() && self.config.private_key.is_none();

        let mut cmd = if use_sshpass {
            let mut wrapped = Command::new("sshpass");
            wrapped.arg("-p").arg(self.config.password.as_ref().unwrap());
            wrapped.arg("ssh");
            for arg in ssh_args {
                wrapped.arg(arg);
            }
            wrapped
        } else {
            let mut cmd = Command::new("ssh");
            for arg in ssh_args {
                cmd.arg(arg);
            }
            cmd
        };

        if let Some(rc) = remote_cmd {
            cmd.arg(rc);
        }

        cmd
    }
}

#[async_trait]
impl Environment for SSHEnvironment {
    fn name(&self) -> &str {
        "ssh"
    }

    fn description(&self) -> String {
        format!(
            "Execute commands on remote host via SSH: {}@{}:{}",
            self.config.user, self.config.host, self.config.port
        )
    }

    fn environment_type(&self) -> crate::manager::EnvironmentType {
        crate::manager::EnvironmentType::SSH
    }

    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout_dur: Option<Duration>,
        _env_vars: Option<&HashMap<String, String>>,
    ) -> Result<ExecutionResult, EnvironmentError> {
        let remote_cwd = cwd.map(PathBuf::from).unwrap_or_else(|| self.config.working_directory.clone());

        // 构建远程命令：cd 到工作目录后执行
        let remote_cmd = if remote_cwd.to_string_lossy() != "." {
            format!("cd {} && {} {}", remote_cwd.display(), command, args.join(" "))
        } else {
            format!("{} {}", command, args.join(" "))
        };

        let mut cmd = self.build_command(Some(&remote_cmd));
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let cmd_str = format!("ssh {}@{} '{}'", self.config.user, self.config.host, remote_cmd);
        tracing::debug!("[SSHEnv] Executing: {}", cmd_str);

        let output = if let Some(dur) = timeout_dur {
            timeout(dur, cmd.output())
                .await
                .map_err(|_| EnvironmentError::Timeout(format!("SSH command timed out after {:?}", dur)))?
                .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?
        } else {
            cmd.output()
                .await
                .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // 检测认证失败
        if stderr.contains("Permission denied") || stderr.contains("Authentication failed") {
            return Err(EnvironmentError::Authentication(format!(
                "SSH authentication failed for {}@{}",
                self.config.user, self.config.host
            )));
        }

        // 检测连接失败
        if stderr.contains("Connection refused") || stderr.contains("No route to host") {
            return Err(EnvironmentError::Connection(format!(
                "Cannot connect to {}:{}",
                self.config.host, self.config.port
            )));
        }

        Ok(ExecutionResult::new(cmd_str, output.status.code(), stdout, stderr))
    }

    async fn read_file(&self, path: &Path) -> Result<String, EnvironmentError> {
        let remote_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        let mut cmd = self.build_command(Some(&format!("cat '{}'", remote_path.display())));
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("No such file") {
                return Err(EnvironmentError::PathNotFound(remote_path.display().to_string()));
            }
            return Err(EnvironmentError::Execution(format!("Failed to read file: {}", err)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<(), EnvironmentError> {
        let remote_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        // 使用 base64 编码避免转义问题
        let encoded = base64_encode(content.as_bytes());
        let write_cmd = format!(
            "mkdir -p $(dirname '{}') && echo '{}' | base64 -d > '{}'",
            remote_path.display(),
            encoded,
            remote_path.display()
        );

        let mut cmd = self.build_command(Some(&write_cmd));
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(EnvironmentError::Execution(format!("Failed to write file: {}", err)));
        }

        Ok(())
    }

    async fn exists(&self, path: &Path) -> Result<bool, EnvironmentError> {
        let remote_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        let mut cmd = self.build_command(Some(&format!("test -e '{}'", remote_path.display())));

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?;

        Ok(output.status.success())
    }

    async fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, EnvironmentError> {
        let remote_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.config.working_directory.join(path)
        };

        let mut cmd = self.build_command(Some(&format!("ls -1 '{}'", remote_path.display())));
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if err.contains("No such file") {
                return Err(EnvironmentError::PathNotFound(remote_path.display().to_string()));
            }
            return Err(EnvironmentError::Execution(format!("Failed to list directory: {}", err)));
        }

        let mut entries = Vec::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        for name in stdout.lines() {
            if name.is_empty() {
                continue;
            }
            let entry_path = remote_path.join(name);
            let stat_cmd = format!("stat -c '%F' '{}'", entry_path.display());

            let mut stat_ssh = self.build_command(Some(&stat_cmd));
            stat_ssh.stdout(Stdio::piped());

            let stat_out = stat_ssh
                .output()
                .await
                .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?;

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
        let mut cmd = self.build_command(Some(&format!("which {}", command)));
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| EnvironmentError::Connection(format!("SSH connection failed: {}", e)))?;

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

fn base64_encode(input: &[u8]) -> String {
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
