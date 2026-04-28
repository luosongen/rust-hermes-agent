//! 文件检查点系统 — 基于 shadow git 仓库的文件快照和回滚
//!
//! 使用 GIT_DIR / GIT_WORK_TREE 环境变量将 git 操作重定向到 shadow 仓库，
//! 避免在用户项目中产生 .git 状态污染。

mod manager;

pub use manager::{CheckpointEntry, CheckpointError, CheckpointManager};
