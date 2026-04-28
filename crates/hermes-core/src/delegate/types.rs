//! Delegate types — parameters and result structures for subagent delegation.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Maximum delegation depth (parent=0, child=1, grandchild=2).
pub const MAX_DELEGATION_DEPTH: u8 = 2;

/// Maximum concurrent child agents in batch mode.
pub const DEFAULT_MAX_CONCURRENT: usize = 3;

/// Default max iterations per subagent.
pub const DEFAULT_MAX_ITERATIONS: u32 = 50;

/// Default timeout in seconds.
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 300;

/// Tools always stripped from subagents.
pub const BLOCKED_TOOLS: &[&str] = &[
    "delegate",
    "clarify",
    "memory",
    "send_message",
    "execute_code",
];

// =============================================================================
// Parameter types
// =============================================================================

/// Single task delegation parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateParams {
    pub goal: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub toolsets: Option<Vec<String>>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// 任务 ID（用于进度报告）
    #[serde(default)]
    pub task_id: Option<String>,
}

/// Batch delegation parameters (parallel execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDelegateParams {
    pub tasks: Vec<DelegateTask>,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
}

/// Individual task within a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateTask {
    pub goal: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub toolsets: Option<Vec<String>>,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// 任务 ID（用于进度报告）
    #[serde(default)]
    pub task_id: Option<String>,
}

fn default_max_iterations() -> u32 {
    DEFAULT_MAX_ITERATIONS
}

fn default_max_concurrent() -> usize {
    DEFAULT_MAX_CONCURRENT
}

// =============================================================================
// Progress reporting
// =============================================================================

/// 进度报告通道
pub type ProgressSender = broadcast::Sender<DelegateProgress>;

/// 进度报告接收器
pub type ProgressReceiver = broadcast::Receiver<DelegateProgress>;

/// 创建进度报告通道
pub fn create_progress_channel() -> (ProgressSender, ProgressReceiver) {
    broadcast::channel(16)
}

/// 委托任务进度报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateProgress {
    /// 任务 ID
    pub task_id: String,
    /// 当前状态
    pub status: DelegateProgressStatus,
    /// 状态消息
    pub message: String,
    /// 进度百分比 (0-100)
    pub percentage: Option<u8>,
    /// 已调用工具次数
    pub tool_calls: u32,
    /// 已消耗 API 调用次数
    pub api_calls: u32,
    /// 已运行时间（毫秒）
    pub elapsed_ms: u64,
}

/// 进度状态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DelegateProgressStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已超时
    Timeout,
    /// 已取消
    Cancelled,
}

// =============================================================================
// Result types
// =============================================================================

/// Delegation operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateResult {
    pub status: DelegateStatus,
    pub summary: String,
    pub api_calls: u32,
    pub duration_ms: u64,
    pub model: String,
    #[serde(default)]
    pub exit_reason: String,
    #[serde(default)]
    pub tool_trace: Vec<ToolTraceEntry>,
    /// 任务 ID
    #[serde(default)]
    pub task_id: Option<String>,
}

/// Status of a delegation task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DelegateStatus {
    Completed,
    Failed,
    Interrupted,
    Error,
    Timeout,
}

impl std::fmt::Display for DelegateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelegateStatus::Completed => write!(f, "completed"),
            DelegateStatus::Failed => write!(f, "failed"),
            DelegateStatus::Interrupted => write!(f, "interrupted"),
            DelegateStatus::Error => write!(f, "error"),
            DelegateStatus::Timeout => write!(f, "timeout"),
        }
    }
}

/// Tool call trace entry for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolTraceEntry {
    pub tool: String,
    pub args_bytes: usize,
    pub result_bytes: usize,
    pub status: String,
}

/// Batch result containing results from all tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDelegateResult {
    pub results: Vec<DelegateResult>,
    pub total_duration_ms: u64,
    /// 成功任务数
    pub success_count: usize,
    /// 失败任务数
    pub failed_count: usize,
}

impl BatchDelegateResult {
    pub fn new(results: Vec<DelegateResult>) -> Self {
        let success_count = results.iter().filter(|r| r.status == DelegateStatus::Completed).count();
        let failed_count = results.len() - success_count;
        let total_duration_ms = results.iter().map(|r| r.duration_ms).sum();

        Self {
            results,
            total_duration_ms,
            success_count,
            failed_count,
        }
    }
}
