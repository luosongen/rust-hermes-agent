//! 安全相关配置 — YOLO 模式和 Fast 模式

use serde::{Deserialize, Serialize};

/// 安全配置
///
/// 控制危险命令审批绕过（YOLO）和快速模式（Fast API processing）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafetyConfig {
    /// YOLO 模式 — 跳过所有危险命令审批检查
    #[serde(default)]
    pub yolo_mode: bool,
    /// Fast 模式 — 使用优先级处理（如 OpenAI Priority / Anthropic Fast）
    #[serde(default)]
    pub fast_mode: bool,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            yolo_mode: false,
            fast_mode: false,
        }
    }
}
