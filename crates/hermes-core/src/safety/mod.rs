//! 安全系统模块
//!
//! 提供命令审批、路径安全检查等安全功能。
//!
//! ## 模块结构
//! - `command_analyzer` — 危险命令分析器
//! - `path_security` — 路径安全检查器
//! - `approval` — 审批处理器
//!
//! ## 使用方式
//!
//! ### 命令审批
//! ```ignore
//! use hermes_core::safety::{CommandAnalyzer, ApprovalManager, ApprovalDecision};
//!
//! let analyzer = CommandAnalyzer::new();
//! let manager = ApprovalManager::interactive();
//!
//! let risk_info = analyzer.get_risk_info("rm -rf /");
//! if risk_info.requires_approval {
//!     match manager.approve_command("rm -rf /", &risk_info) {
//!         ApprovalDecision::Allow => { /* 执行命令 */ },
//!         ApprovalDecision::Deny => { /* 拒绝执行 */ },
//!         ApprovalDecision::AllowAlways(pattern) => { /* 允许并记住 */ },
//!     }
//! }
//! ```
//!
//! ### 路径安全检查
//! ```ignore
//! use hermes_core::safety::PathSecurityChecker;
//!
//! let checker = PathSecurityChecker::default();
//!
//! // 检查写入是否安全
//! match checker.check_write(Path::new("/etc/passwd")) {
//!     Ok(()) => { /* 安全 */ },
//!     Err(e) => { /* 不安全: e */ },
//! }
//! ```

pub mod approval;
pub mod command_analyzer;
pub mod path_security;

// Re-exports
pub use approval::{
    ApprovalDecision, ApprovalHandler, ApprovalManager, ApprovalRequest, InteractiveApprovalHandler,
    OperationType, SilentApprovalHandler,
};
pub use command_analyzer::{CommandAnalyzer, RiskInfo, RiskLevel};
pub use path_security::{PathSecurityChecker, PathSecurityConfig, PathSecurityError};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration() {
        // 创建组件
        let analyzer = CommandAnalyzer::new();
        let manager = ApprovalManager::yolo(); // YOLO 模式用于测试

        // 分析命令
        let info = analyzer.get_risk_info("rm -rf ./build");
        assert!(info.requires_approval);

        // 审批命令
        let decision = manager.approve_command("rm -rf ./build", &info);
        assert_eq!(decision, ApprovalDecision::Allow);
    }
}
