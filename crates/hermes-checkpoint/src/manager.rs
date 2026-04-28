//! CheckpointManager — shadow git 仓库文件快照管理器

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tokio::process::Command;

/// 单个检查点条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointEntry {
    /// 短 commit hash
    pub commit_hash: String,
    /// Unix 时间戳
    pub timestamp: i64,
    /// 提交消息
    pub message: String,
}

/// 检查点错误
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("git 未找到，请确认 git 已安装")]
    GitNotFound,
    #[error("git 命令执行失败: {0}")]
    GitError(String),
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),
}

/// 文件检查点管理器
///
/// 使用 shadow git 仓库（通过 GIT_DIR / GIT_WORK_TREE 环境变量）
/// 在 `~/.config/hermes-agent/checkpoints/<sha256(路径)[:16]>/` 下维护 git 状态。
#[derive(Debug)]
pub struct CheckpointManager {
    /// 检查点基础目录
    #[allow(dead_code)]
    base_dir: PathBuf,
    /// 去重集合: (规范化工作目录路径, 回合号)
    /// 同一目录同一回合只创建一次快照
    seen: Mutex<HashSet<(String, u64)>>,
    /// 当前回合计数
    turn: std::sync::atomic::AtomicU64,
}

impl CheckpointManager {
    /// 创建新的 CheckpointManager
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            seen: Mutex::new(HashSet::new()),
            turn: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// 对单个文件创建检查点快照
    pub async fn snapshot_file(
        &self,
        file_path: &Path,
        working_dir: &Path,
    ) -> Result<(), CheckpointError> {
        let working_dir = self.canonicalize(working_dir)?;
        let key = working_dir.to_string_lossy().to_string();
        let turn = self.turn.load(std::sync::atomic::Ordering::Relaxed);

        // 去重：同一目录同一回合只快照一次
        {
            let mut seen = self.seen.lock().unwrap();
            if !seen.insert((key.clone(), turn)) {
                return Ok(());
            }
        }

        // 计算相对路径
        let relative = file_path
            .strip_prefix(&working_dir)
            .unwrap_or(file_path);

        let shadow_dir = Self::shadow_repo_path(&working_dir);

        // 首次使用时初始化 shadow 仓库
        if !shadow_dir.join(".git").exists() {
            self.init_shadow_repo(&shadow_dir, &working_dir).await?;
        }

        let relative_str = relative.to_string_lossy();

        // git add <relative_path>
        self.git_in_shadow(&shadow_dir, &working_dir, &["add", "--", &relative_str])
            .await?;

        // 检查是否有变更
        let diff_output = self
            .git_in_shadow_with_output(
                &shadow_dir,
                &working_dir,
                &["diff", "--cached", "--quiet", "--", &relative_str],
            )
            .await;
        // diff --quiet: exit 0 = no changes, exit 1 = has changes
        if diff_output.map_or(false, |o| o.status.success()) {
            // 无变更，跳过
            return Ok(());
        }

        let reason = format!(
            "checkpoint: {} @ turn {}",
            relative_str, turn
        );
        self.git_in_shadow(
            &shadow_dir,
            &working_dir,
            &["commit", "-m", &reason, "--allow-empty-message", "--no-verify"],
        )
        .await?;

        tracing::debug!("检查点已创建: {} (turn {})", relative_str, turn);
        Ok(())
    }

    /// 对整个工作目录创建检查点快照
    pub async fn snapshot_working_dir(
        &self,
        working_dir: &Path,
    ) -> Result<(), CheckpointError> {
        let working_dir = self.canonicalize(working_dir)?;
        let key = working_dir.to_string_lossy().to_string();
        let turn = self.turn.load(std::sync::atomic::Ordering::Relaxed);

        {
            let mut seen = self.seen.lock().unwrap();
            if !seen.insert((key.clone(), turn)) {
                return Ok(());
            }
        }

        let shadow_dir = Self::shadow_repo_path(&working_dir);
        if !shadow_dir.join(".git").exists() {
            self.init_shadow_repo(&shadow_dir, &working_dir).await?;
        }

        // git add -A
        self.git_in_shadow(&shadow_dir, &working_dir, &["add", "-A"])
            .await?;

        let diff_output = self
            .git_in_shadow_with_output(
                &shadow_dir,
                &working_dir,
                &["diff", "--cached", "--quiet"],
            )
            .await;
        if diff_output.map_or(false, |o| o.status.success()) {
            return Ok(());
        }

        let reason = format!("checkpoint: working_dir @ turn {}", turn);
        self.git_in_shadow(
            &shadow_dir,
            &working_dir,
            &["commit", "-m", &reason, "--allow-empty-message", "--no-verify"],
        )
        .await?;

        tracing::debug!("工作目录检查点已创建 (turn {})", turn);
        Ok(())
    }

    /// 列出检查点历史
    pub async fn list_checkpoints(
        &self,
        working_dir: &Path,
        file_path: Option<&Path>,
    ) -> Result<Vec<CheckpointEntry>, CheckpointError> {
        let working_dir = self.canonicalize(working_dir)?;
        let shadow_dir = Self::shadow_repo_path(&working_dir);

        if !shadow_dir.join(".git").exists() {
            return Ok(Vec::new());
        }

        let mut args = vec!["log", "--oneline", "--format=%H %at %s"];
        // 保存文件路径的字符串，确保生命周期覆盖 args 的使用
        let file_path_str: Option<String> = file_path.map(|fp| {
            fp.strip_prefix(&working_dir)
                .unwrap_or(fp)
                .to_string_lossy()
                .to_string()
        });
        if let Some(ref s) = file_path_str {
            args.push("--");
            args.push(s.as_str());
        }

        let output = self
            .git_in_shadow_with_output(&shadow_dir, &working_dir, &args)
            .await
            .map_err(|e| CheckpointError::GitError(format!("git log 失败: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<CheckpointEntry> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                // 格式: <hash> <timestamp> <message>
                let parts: Vec<&str> = line.splitn(3, ' ').collect();
                if parts.len() < 3 {
                    return None;
                }
                let commit_hash = parts[0][..8.min(parts[0].len())].to_string();
                let timestamp: i64 = parts[1].parse().ok()?;
                let message = parts[2].to_string();

                Some(CheckpointEntry {
                    commit_hash,
                    timestamp,
                    message,
                })
            })
            .collect();

        // 反转顺序使最新的在前
        let mut entries = entries;
        entries.reverse();
        Ok(entries)
    }

    /// 回滚到指定检查点
    ///
    /// 回滚前会自动创建 pre-rollback 快照。
    pub async fn restore_checkpoint(
        &self,
        working_dir: &Path,
        commit_hash: &str,
        file_path: Option<&Path>,
    ) -> Result<(), CheckpointError> {
        let working_dir = self.canonicalize(working_dir)?;
        let shadow_dir = Self::shadow_repo_path(&working_dir);

        if !shadow_dir.join(".git").exists() {
            return Err(CheckpointError::GitError("没有检查点历史记录".into()));
        }

        // 安全检查：commit_hash 必须是有效的 hex 字符串
        if commit_hash.is_empty() || commit_hash.starts_with('-') {
            return Err(CheckpointError::GitError("无效的 commit hash".into()));
        }
        if !commit_hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(CheckpointError::GitError("无效的 commit hash 格式".into()));
        }

        // 回滚前创建 pre-rollback 快照
        let _ = self
            .snapshot_working_dir(&working_dir)
            .await;

        let target = if let Some(fp) = file_path {
            fp.strip_prefix(&working_dir)
                .unwrap_or(fp)
                .to_string_lossy()
                .to_string()
        } else {
            ".".to_string()
        };

        let output = self
            .git_in_shadow_with_output(
                &shadow_dir,
                &working_dir,
                &["checkout", commit_hash, "--", &target],
            )
            .await
            .map_err(|e| CheckpointError::GitError(format!("git checkout 失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CheckpointError::GitError(format!(
                "回滚失败: {}",
                stderr
            )));
        }

        tracing::info!("已回滚到检查点 {}", commit_hash);
        Ok(())
    }

    /// 递增回合计数器（每个 Agent 迭代后调用），清零去重集合
    pub fn advance_turn(&self) {
        self.turn
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.seen.lock().unwrap().clear();
    }

    // ==========================================================================
    // 内部方法
    // ==========================================================================

    /// 计算 shadow 仓库路径
    fn shadow_repo_path(working_dir: &Path) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(working_dir.to_string_lossy().as_bytes());
        let hash = hex::encode(&hasher.finalize()[..8]); // 前 8 字节 = 16 hex chars
        let base = dirs_next(); // 使用平台默认配置目录
        base.join("hermes-agent")
            .join("checkpoints")
            .join(hash)
    }

    /// 初始化 shadow 仓库
    async fn init_shadow_repo(
        &self,
        shadow_dir: &Path,
        working_dir: &Path,
    ) -> Result<(), CheckpointError> {
        tokio::fs::create_dir_all(shadow_dir).await?;

        // git init
        let output = Command::new("git")
            .arg("init")
            .arg("--quiet")
            .current_dir(shadow_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|_| CheckpointError::GitNotFound)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CheckpointError::GitError(format!(
                "git init 失败: {}",
                stderr
            )));
        }

        // 设置 commit 用户信息
        let _ = Command::new("git")
            .args(["config", "user.email", "hermes@local"])
            .current_dir(shadow_dir)
            .output()
            .await;

        let _ = Command::new("git")
            .args(["config", "user.name", "Hermes Checkpoint"])
            .current_dir(shadow_dir)
            .output()
            .await;

        // 写入默认排除规则
        let exclude_file = shadow_dir.join(".git").join("info").join("exclude");
        if let Some(parent) = exclude_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let excludes = "node_modules\ndist\nbuild\n.env\n__pycache__\n.git\n.DS_Store\n";
        tokio::fs::write(&exclude_file, excludes).await?;

        // 标记文件，记录原始工作目录
        let marker = shadow_dir.join("HERMES_WORKDIR");
        tokio::fs::write(&marker, working_dir.to_string_lossy().as_bytes()).await?;

        Ok(())
    }

    /// 在 shadow 仓库中执行 git 命令，忽略输出
    async fn git_in_shadow(
        &self,
        shadow_dir: &Path,
        working_dir: &Path,
        args: &[&str],
    ) -> Result<(), CheckpointError> {
        let output = self
            .git_in_shadow_with_output(shadow_dir, working_dir, args)
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CheckpointError::GitError(format!(
                "git {} 失败: {}",
                args.join(" "),
                stderr
            )));
        }
        Ok(())
    }

    /// 在 shadow 仓库中执行 git 命令，返回完整输出
    async fn git_in_shadow_with_output(
        &self,
        shadow_dir: &Path,
        working_dir: &Path,
        args: &[&str],
    ) -> Result<std::process::Output, CheckpointError> {
        let shadow_git = shadow_dir.join(".git");
        Command::new("git")
            .args(args)
            .current_dir(working_dir)
            .env("GIT_DIR", &shadow_git)
            .env("GIT_WORK_TREE", working_dir)
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_NAMESPACE")
            .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|_| CheckpointError::GitNotFound)
    }

    /// 规范化路径
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, CheckpointError> {
        path.canonicalize().map_err(CheckpointError::Io)
    }
}

/// 获取平台配置目录
fn dirs_next() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs_next_fallback()
    }
    #[cfg(not(target_os = "macos"))]
    {
        dirs_next_fallback()
    }
}

fn dirs_next_fallback() -> PathBuf {
    if let Some(d) = ::std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(d);
    }
    if let Some(home) = ::std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config");
    }
    PathBuf::from("~/.config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as SyncCommand;

    fn ensure_git() -> bool {
        SyncCommand::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn test_shadow_repo_path_is_deterministic() {
        let path1 = CheckpointManager::shadow_repo_path(Path::new("/tmp/test"));
        let path2 = CheckpointManager::shadow_repo_path(Path::new("/tmp/test"));
        assert_eq!(path1, path2);

        let path3 = CheckpointManager::shadow_repo_path(Path::new("/tmp/other"));
        assert_ne!(path1, path3);
    }

    #[tokio::test]
    async fn test_checkpoint_snapshot_and_list() {
        if !ensure_git() {
            eprintln!("跳过测试: git 未安装");
            return;
        }

        let tmp = tempfile::tempdir().expect("创建临时目录");
        let work = tmp.path().join("work");
        std::fs::create_dir_all(&work).unwrap();

        // 创建测试文件
        std::fs::write(work.join("test.txt"), "hello").unwrap();

        let base = tmp.path().join("checkpoints");
        let cm = CheckpointManager::new(base.clone());

        // 创建快照
        cm.snapshot_file(&work.join("test.txt"), &work)
            .await
            .expect("快照创建");

        // 修改文件
        std::fs::write(work.join("test.txt"), "modified").unwrap();

        // 再创建一次快照
        cm.advance_turn();
        cm.snapshot_file(&work.join("test.txt"), &work)
            .await
            .expect("第二次快照");

        // 列出检查点
        let entries = cm.list_checkpoints(&work, None).await.expect("列出检查点");
        assert_eq!(entries.len(), 2);

        // 回滚到第一个检查点
        let first_hash = entries[0].commit_hash.clone();
        cm.restore_checkpoint(&work, &first_hash, None)
            .await
            .expect("回滚");

        let content = std::fs::read_to_string(work.join("test.txt")).unwrap();
        assert_eq!(content, "hello");
    }

    #[tokio::test]
    async fn test_dedup_same_turn() {
        if !ensure_git() {
            eprintln!("跳过测试: git 未安装");
            return;
        }

        let tmp = tempfile::tempdir().expect("创建临时目录");
        let work = tmp.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("a.txt"), "a").unwrap();
        std::fs::write(work.join("b.txt"), "b").unwrap();

        let base = tmp.path().join("checkpoints");
        let cm = CheckpointManager::new(base.clone());

        // 同一 turn 内对同一目录快照两次
        cm.snapshot_file(&work.join("a.txt"), &work).await.unwrap();
        cm.snapshot_file(&work.join("b.txt"), &work).await.unwrap();

        // 由于去重，第二次应该跳过 — 只创建了一个 commit
        let entries = cm.list_checkpoints(&work, None).await.unwrap();
        assert_eq!(entries.len(), 1, "同一 turn 内只应创建一个快照");

        // 新 turn 后可以再创建
        cm.advance_turn();
        std::fs::write(work.join("b.txt"), "b2").unwrap();
        cm.snapshot_file(&work.join("b.txt"), &work).await.unwrap();
        let entries = cm.list_checkpoints(&work, None).await.unwrap();
        assert_eq!(entries.len(), 2);
    }
}
