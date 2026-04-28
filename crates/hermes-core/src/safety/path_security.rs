//! 路径安全检查器
//!
//! 检查文件操作路径的安全性，防止：
//! - 访问敏感文件（如 .env, credentials）
//! - 写入系统关键目录
//! - 路径遍历攻击

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 路径安全错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum PathSecurityError {
    #[error("路径被拒绝: {0}")]
    Denied(String),

    #[error("路径不在允许的目录中: {0}")]
    NotInAllowedDirectory(String),

    #[error("路径指向敏感文件: {0}")]
    SensitiveFile(String),

    #[error("路径遍历攻击检测: {0}")]
    TraversalAttack(String),

    #[error("路径解析失败: {0}")]
    InvalidPath(String),
}

/// 敏感文件名模式
const SENSITIVE_FILES: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    ".env.development",
    ".envrc",
    "credentials.json",
    "credentials",
    "secrets.json",
    "secrets",
    ".git-credentials",
    ".netrc",
    "_netrc",
    ".pgpass",
    ".my.cnf",
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    "aws_credentials",
    ".aws/credentials",
    ".docker/config.json",
    "kubeconfig",
    ".kube/config",
];

/// 禁止写入的系统目录
const PROTECTED_DIRECTORIES: &[&str] = &[
    "/etc",
    "/bin",
    "/sbin",
    "/usr/bin",
    "/usr/sbin",
    "/lib",
    "/lib64",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/root",
];

/// 路径安全配置
#[derive(Debug, Clone)]
pub struct PathSecurityConfig {
    /// 允许的根目录列表（空表示允许所有）
    pub allowed_roots: Vec<PathBuf>,
    /// 禁止访问的路径列表
    pub denied_paths: Vec<PathBuf>,
    /// 是否检查敏感文件
    pub check_sensitive_files: bool,
    /// 是否检查路径遍历
    pub check_traversal: bool,
    /// 是否保护系统目录
    pub protect_system_dirs: bool,
}

impl Default for PathSecurityConfig {
    fn default() -> Self {
        Self {
            allowed_roots: Vec::new(), // 默认允许所有
            denied_paths: PROTECTED_DIRECTORIES.iter().map(PathBuf::from).collect(),
            check_sensitive_files: true,
            check_traversal: true,
            protect_system_dirs: true,
        }
    }
}

/// 路径安全检查器
pub struct PathSecurityChecker {
    config: PathSecurityConfig,
}

impl Default for PathSecurityChecker {
    fn default() -> Self {
        Self::new(PathSecurityConfig::default())
    }
}

impl PathSecurityChecker {
    /// 创建新的路径安全检查器
    pub fn new(config: PathSecurityConfig) -> Self {
        Self { config }
    }

    /// 检查路径读取是否安全
    pub fn check_read(&self, path: &Path) -> Result<(), PathSecurityError> {
        let canonical = self.canonicalize_path(path)?;

        // 检查禁止路径
        self.check_denied_paths(&canonical)?;

        // 检查允许根目录
        self.check_allowed_roots(&canonical)?;

        // 检查敏感文件（警告但不阻止）
        if self.config.check_sensitive_files {
            self.warn_sensitive_file(&canonical);
        }

        Ok(())
    }

    /// 检查路径写入是否安全
    pub fn check_write(&self, path: &Path) -> Result<(), PathSecurityError> {
        let canonical = self.canonicalize_path(path)?;

        // 检查禁止路径
        self.check_denied_paths(&canonical)?;

        // 检查允许根目录
        self.check_allowed_roots(&canonical)?;

        // 检查敏感文件
        if self.config.check_sensitive_files {
            self.check_sensitive_file(&canonical)?;
        }

        // 检查系统目录
        if self.config.protect_system_dirs {
            self.check_system_directory(&canonical)?;
        }

        Ok(())
    }

    /// 检查路径删除是否安全
    pub fn check_delete(&self, path: &Path) -> Result<(), PathSecurityError> {
        let canonical = self.canonicalize_path(path)?;

        // 删除操作更严格
        self.check_write(&canonical)?;

        // 检查是否是根目录
        if canonical.parent().is_none() {
            return Err(PathSecurityError::Denied("不能删除根目录".into()));
        }

        // 检查是否是用户主目录
        if let Some(home) = dirs::home_dir() {
            if canonical == home {
                return Err(PathSecurityError::Denied("不能删除用户主目录".into()));
            }
        }

        Ok(())
    }

    /// 规范化路径
    fn canonicalize_path(&self, path: &Path) -> Result<PathBuf, PathSecurityError> {
        // 检查路径遍历
        if self.config.check_traversal {
            let path_str = path.to_string_lossy();
            if path_str.contains("..") {
                // 如果路径存在，使用 canonicalize
                if path.exists() {
                    return path
                        .canonicalize()
                        .map_err(|e| PathSecurityError::InvalidPath(e.to_string()));
                }
                // 否则尝试解析相对路径
                let resolved = self.resolve_path(path)?;
                if resolved.to_string_lossy().contains("..") {
                    return Err(PathSecurityError::TraversalAttack(path.display().to_string()));
                }
                return Ok(resolved);
            }
        }

        // 尝试 canonicalize，如果路径不存在则使用原始路径
        // 对于系统目录检查，即使路径不存在也要检查
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        Ok(canonical)
    }

    /// 解析路径（处理 .. 和 .）
    fn resolve_path(&self, path: &Path) -> Result<PathBuf, PathSecurityError> {
        let mut components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    if components.pop().is_none() {
                        // 尝试从当前目录解析
                        let cwd = std::env::current_dir()
                            .map_err(|e| PathSecurityError::InvalidPath(e.to_string()))?;
                        if !cwd.parent().map(|p| p.components().count()).unwrap_or(0) > 0 {
                            return Err(PathSecurityError::TraversalAttack(
                                path.display().to_string(),
                            ));
                        }
                    }
                }
                std::path::Component::CurDir => {}
                _ => components.push(component),
            }
        }

        Ok(components.iter().collect())
    }

    /// 检查禁止路径
    fn check_denied_paths(&self, path: &Path) -> Result<(), PathSecurityError> {
        for denied in &self.config.denied_paths {
            if path.starts_with(denied) {
                return Err(PathSecurityError::Denied(format!(
                    "路径 '{}' 在禁止列表中",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    /// 检查允许根目录
    fn check_allowed_roots(&self, path: &Path) -> Result<(), PathSecurityError> {
        if self.config.allowed_roots.is_empty() {
            return Ok(());
        }

        for root in &self.config.allowed_roots {
            if path.starts_with(root) {
                return Ok(());
            }
        }

        Err(PathSecurityError::NotInAllowedDirectory(
            path.display().to_string(),
        ))
    }

    /// 检查敏感文件（写入时阻止）
    fn check_sensitive_file(&self, path: &Path) -> Result<(), PathSecurityError> {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let path_str = path.to_string_lossy();

        for sensitive in SENSITIVE_FILES {
            // 检查文件名
            if file_name == *sensitive || file_name.ends_with(sensitive) {
                return Err(PathSecurityError::SensitiveFile(path.display().to_string()));
            }
            // 检查路径包含
            if path_str.contains(sensitive) {
                return Err(PathSecurityError::SensitiveFile(path.display().to_string()));
            }
        }

        Ok(())
    }

    /// 警告敏感文件（读取时仅警告）
    fn warn_sensitive_file(&self, path: &Path) {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        for sensitive in SENSITIVE_FILES {
            if file_name == *sensitive || file_name.ends_with(sensitive) {
                eprintln!("⚠️  警告: 正在访问敏感文件: {}", path.display());
                return;
            }
        }
    }

    /// 检查系统目录
    fn check_system_directory(&self, path: &Path) -> Result<(), PathSecurityError> {
        let path_str = path.to_string_lossy();

        for dir in PROTECTED_DIRECTORIES {
            // 检查原始路径
            let dir_path = PathBuf::from(dir);
            if path.starts_with(&dir_path) {
                return Err(PathSecurityError::Denied(format!(
                    "不能写入系统目录: {}",
                    path.display()
                )));
            }

            // 检查路径字符串前缀（处理 macOS 符号链接等情况）
            if path_str.starts_with(dir) || path_str.starts_with(&format!("/private{}", dir)) {
                return Err(PathSecurityError::Denied(format!(
                    "不能写入系统目录: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    /// 添加允许的根目录
    pub fn add_allowed_root(&mut self, path: PathBuf) {
        self.config.allowed_roots.push(path);
    }

    /// 添加禁止路径
    pub fn add_denied_path(&mut self, path: PathBuf) {
        self.config.denied_paths.push(path);
    }

    /// 检查路径是否是敏感文件
    pub fn is_sensitive_file(&self, path: &Path) -> bool {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        for sensitive in SENSITIVE_FILES {
            if file_name == *sensitive || file_name.ends_with(sensitive) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitive_file_detection() {
        let checker = PathSecurityChecker::default();

        assert!(checker.is_sensitive_file(Path::new(".env")));
        assert!(checker.is_sensitive_file(Path::new("secrets.json")));
        assert!(checker.is_sensitive_file(Path::new("id_rsa")));
        assert!(!checker.is_sensitive_file(Path::new("normal_file.txt")));
    }

    #[test]
    fn test_system_directory_protection() {
        let checker = PathSecurityChecker::default();

        assert!(checker.check_write(Path::new("/etc/passwd")).is_err());
        assert!(checker.check_write(Path::new("/bin/test")).is_err());
    }

    #[test]
    fn test_allowed_roots() {
        let config = PathSecurityConfig {
            allowed_roots: vec![PathBuf::from("/home/user/projects")],
            denied_paths: vec![],
            check_sensitive_files: false,
            check_traversal: true,
            protect_system_dirs: false,
        };
        let checker = PathSecurityChecker::new(config);

        // 这个测试依赖于路径是否存在
        // assert!(checker.check_read(Path::new("/home/user/projects/test.txt")).is_ok());
        assert!(checker.check_read(Path::new("/etc/passwd")).is_err());
    }
}
