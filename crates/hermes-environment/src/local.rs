//! LocalEnvironment — 本地进程执行后端
//!
//! 在 Agent 所在主机上直接执行命令，是默认的后端实现。

use crate::{DirEntry, Environment, EnvironmentError, ExecutionResult};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// 本地执行环境
///
/// 在本地主机上直接 spawn 进程执行命令。
#[derive(Debug, Clone)]
pub struct LocalEnvironment {
    /// 默认工作目录
    working_directory: PathBuf,
}

impl LocalEnvironment {
    /// 创建新的本地环境
    pub fn new(working_directory: impl Into<PathBuf>) -> Self {
        Self {
            working_directory: working_directory.into(),
        }
    }

    /// 获取默认工作目录
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }
}

#[async_trait]
impl Environment for LocalEnvironment {
    fn name(&self) -> &str {
        "local"
    }

    fn description(&self) -> String {
        "Execute commands on the local machine".to_string()
    }

    fn environment_type(&self) -> crate::manager::EnvironmentType {
        crate::manager::EnvironmentType::Local
    }

    async fn execute(
        &self,
        command: &str,
        args: &[&str],
        cwd: Option<&Path>,
        timeout_dur: Option<Duration>,
        env_vars: Option<&HashMap<String, String>>,
    ) -> Result<ExecutionResult, EnvironmentError> {
        let work_dir = cwd.map(PathBuf::from).unwrap_or_else(|| self.working_directory.clone());

        let mut cmd = Command::new(command);
        cmd.args(args)
            .current_dir(&work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(envs) = env_vars {
            for (key, value) in envs {
                cmd.env(key, value);
            }
        }

        let cmd_str = format!("{} {}", command, args.join(" "));
        tracing::debug!("[LocalEnv] Executing: {} in {:?}", cmd_str, work_dir);

        let output = if let Some(dur) = timeout_dur {
            timeout(dur, cmd.output())
                .await
                .map_err(|_| EnvironmentError::Timeout(format!("Command timed out after {:?}", dur)))?
                .map_err(|e| EnvironmentError::Execution(format!("Failed to spawn process: {}", e)))?
        } else {
            cmd.output()
                .await
                .map_err(|e| EnvironmentError::Execution(format!("Failed to spawn process: {}", e)))?
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(ExecutionResult::new(cmd_str, output.status.code(), stdout, stderr))
    }

    async fn read_file(&self, path: &Path) -> Result<String, EnvironmentError> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        };

        tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => EnvironmentError::PathNotFound(full_path.display().to_string()),
                std::io::ErrorKind::PermissionDenied => EnvironmentError::PermissionDenied(full_path.display().to_string()),
                _ => EnvironmentError::Io(e),
            })
    }

    async fn write_file(&self, path: &Path, content: &str) -> Result<(), EnvironmentError> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        };

        // 确保父目录存在
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(EnvironmentError::Io)?;
        }

        tokio::fs::write(&full_path, content)
            .await
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => EnvironmentError::PermissionDenied(full_path.display().to_string()),
                _ => EnvironmentError::Io(e),
            })
    }

    async fn exists(&self, path: &Path) -> Result<bool, EnvironmentError> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        };
        Ok(tokio::fs::metadata(&full_path).await.is_ok())
    }

    async fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, EnvironmentError> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        };

        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&full_path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => EnvironmentError::PathNotFound(full_path.display().to_string()),
            std::io::ErrorKind::PermissionDenied => EnvironmentError::PermissionDenied(full_path.display().to_string()),
            _ => EnvironmentError::Io(e),
        })?;

        while let Some(entry) = dir.next_entry().await.map_err(EnvironmentError::Io)? {
            let meta = entry.metadata().await.map_err(EnvironmentError::Io)?;
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(DirEntry {
                name,
                is_file: meta.is_file(),
                is_dir: meta.is_dir(),
            });
        }

        Ok(entries)
    }

    async fn which(&self, command: &str) -> Result<Option<String>, EnvironmentError> {
        let result = self.execute("which", &[command], None, Some(Duration::from_secs(5)), None).await?;
        if result.success {
            let path = result.stdout.trim();
            if path.is_empty() {
                Ok(None)
            } else {
                Ok(Some(path.to_string()))
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_echo() {
        let env = LocalEnvironment::new(".");
        let result = env.execute("echo", &["hello", "world"], None, None, None).await.unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello world"));
    }

    #[tokio::test]
    async fn test_local_file_ops() {
        let tmpdir = tempfile::tempdir().unwrap();
        let env = LocalEnvironment::new(tmpdir.path());

        // write + read
        env.write_file(Path::new("test.txt"), "hello").await.unwrap();
        let content = env.read_file(Path::new("test.txt")).await.unwrap();
        assert_eq!(content, "hello");

        // exists
        assert!(env.exists(Path::new("test.txt")).await.unwrap());
        assert!(!env.exists(Path::new("nonexistent")).await.unwrap());

        // list_dir
        let entries = env.list_dir(Path::new(".")).await.unwrap();
        assert!(entries.iter().any(|e| e.name == "test.txt"));
    }

    #[tokio::test]
    async fn test_local_timeout() {
        let env = LocalEnvironment::new(".");
        let result = env.execute("sleep", &["10"], None, Some(Duration::from_millis(100)), None).await;
        assert!(matches!(result, Err(EnvironmentError::Timeout(_))));
    }
}
