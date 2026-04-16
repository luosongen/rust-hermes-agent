//! Delegate types — parameters and result structures for subagent delegation.

use serde::{Deserialize, Serialize};

/// Maximum delegation depth (parent=0, child=1, grandchild=2).
pub const MAX_DELEGATION_DEPTH: u8 = 2;

/// Maximum concurrent child agents in batch mode.
pub const DEFAULT_MAX_CONCURRENT: usize = 3;

/// Default max iterations per subagent.
pub const DEFAULT_MAX_ITERATIONS: u32 = 50;

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
}

fn default_max_iterations() -> u32 {
    DEFAULT_MAX_ITERATIONS
}

fn default_max_concurrent() -> usize {
    DEFAULT_MAX_CONCURRENT
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
}

/// Status of a delegation task.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DelegateStatus {
    Completed,
    Failed,
    Interrupted,
    Error,
}

impl std::fmt::Display for DelegateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelegateStatus::Completed => write!(f, "completed"),
            DelegateStatus::Failed => write!(f, "failed"),
            DelegateStatus::Interrupted => write!(f, "interrupted"),
            DelegateStatus::Error => write!(f, "error"),
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
}
