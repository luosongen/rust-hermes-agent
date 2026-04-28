use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Delegation configuration for sub-agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConfig {
    /// 是否启用委托功能
    #[serde(default = "default_delegation_enabled")]
    pub enabled: bool,

    /// 子代理默认人格
    pub default_personality: Option<String>,

    /// 子代理默认模型
    #[serde(default = "default_model")]
    pub default_model: String,

    /// 最大委托深度
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,

    /// 子代理最大 token 数
    pub max_tokens: Option<u32>,

    /// 遇到这些模型时终止委托
    #[serde(default)]
    pub terminate_on_model: Vec<String>,

    /// 最大并行子代理数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,

    /// 单个任务超时（秒）
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,

    /// 默认允许的工具集（空表示全部）
    #[serde(default)]
    pub allowed_tools: Vec<String>,

    /// 禁用的工具列表（黑名单）
    #[serde(default = "default_blocked_tools")]
    pub blocked_tools: Vec<String>,

    /// 进度报告间隔（毫秒）
    #[serde(default = "default_progress_interval")]
    pub progress_report_interval_ms: u64,
}

fn default_delegation_enabled() -> bool {
    false
}

fn default_model() -> String {
    "openai/gpt-4o".to_string()
}

fn default_max_depth() -> u32 {
    2
}

fn default_max_concurrent() -> usize {
    3
}

fn default_timeout_seconds() -> u64 {
    300 // 5 分钟
}

fn default_blocked_tools() -> Vec<String> {
    vec![
        "delegate".to_string(),
        "clarify".to_string(),
        "memory".to_string(),
        "send_message".to_string(),
        "execute_code".to_string(),
    ]
}

fn default_progress_interval() -> u64 {
    1000 // 1 秒
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_personality: None,
            default_model: default_model(),
            max_depth: default_max_depth(),
            max_tokens: None,
            terminate_on_model: Vec::new(),
            max_concurrent: default_max_concurrent(),
            timeout_seconds: default_timeout_seconds(),
            allowed_tools: Vec::new(),
            blocked_tools: default_blocked_tools(),
            progress_report_interval_ms: default_progress_interval(),
        }
    }
}

impl DelegationConfig {
    /// 获取合并后的工具黑名单
    pub fn get_effective_blocked_tools(&self, extra: &[String]) -> HashSet<String> {
        let mut blocked: HashSet<String> = self.blocked_tools.iter().cloned().collect();
        blocked.extend(extra.iter().cloned());
        blocked
    }

    /// 获取有效的允许工具列表
    pub fn get_effective_allowed_tools(&self, requested: Option<&[String]>) -> Option<Vec<String>> {
        match (requested, self.allowed_tools.is_empty()) {
            (Some(tools), _) => Some(tools.to_vec()),
            (None, false) => Some(self.allowed_tools.clone()),
            (None, true) => None, // 允许所有工具
        }
    }
}
