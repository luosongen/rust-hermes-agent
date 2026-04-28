//! 审批处理器
//!
//! 处理危险命令和敏感操作的审批流程。

use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

use super::command_analyzer::{RiskLevel, RiskInfo};
use super::path_security::PathSecurityError;

/// 审批决策
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// 允许执行
    Allow,
    /// 拒绝执行
    Deny,
    /// 添加到永久白名单
    AllowAlways(String),
}

/// 审批请求
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    /// 命令或操作描述
    pub operation: String,
    /// 风险等级
    pub risk_level: RiskLevel,
    /// 风险原因
    pub risk_reason: Option<String>,
    /// 操作类型
    pub operation_type: OperationType,
}

/// 操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Shell 命令执行
    ShellCommand,
    /// 文件写入
    FileWrite,
    /// 文件删除
    FileDelete,
    /// 网络请求
    NetworkRequest,
    /// 其他操作
    Other,
}

/// 审批处理器 Trait
pub trait ApprovalHandler: Send + Sync {
    /// 请求审批
    fn request_approval(&self, request: &ApprovalRequest) -> ApprovalDecision;
}

/// 控制台交互式审批处理器
pub struct InteractiveApprovalHandler {
    /// 永久白名单
    whitelist: HashSet<String>,
    /// 白名单文件路径
    whitelist_path: PathBuf,
    /// YOLO 模式（跳过所有审批）
    yolo_mode: bool,
}

impl InteractiveApprovalHandler {
    /// 创建新的交互式审批处理器
    pub fn new() -> Self {
        let whitelist_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hermes-agent")
            .join("whitelist.txt");

        let mut handler = Self {
            whitelist: HashSet::new(),
            whitelist_path,
            yolo_mode: false,
        };

        // 加载白名单
        let _ = handler.load_whitelist();

        handler
    }

    /// 启用/禁用 YOLO 模式
    pub fn set_yolo_mode(&mut self, enabled: bool) {
        self.yolo_mode = enabled;
    }

    /// 加载白名单
    fn load_whitelist(&mut self) -> io::Result<()> {
        if self.whitelist_path.exists() {
            let content = fs::read_to_string(&self.whitelist_path)?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    self.whitelist.insert(line.to_string());
                }
            }
        }
        Ok(())
    }

    /// 保存白名单
    fn save_whitelist(&self) -> io::Result<()> {
        if let Some(parent) = self.whitelist_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content: String = self.whitelist.iter().cloned().collect::<Vec<_>>().join("\n");
        fs::write(&self.whitelist_path, content)
    }

    /// 检查是否在白名单中
    fn is_in_whitelist(&self, operation: &str) -> bool {
        // 精确匹配
        if self.whitelist.contains(operation) {
            return true;
        }

        // 前缀匹配（用于命令模式）
        for pattern in &self.whitelist {
            if operation.starts_with(pattern) {
                return true;
            }
        }

        false
    }

    /// 添加到白名单
    fn add_to_whitelist(&mut self, pattern: String) {
        self.whitelist.insert(pattern);
        let _ = self.save_whitelist();
    }

    /// 交互式请求审批
    fn interactive_approval(&mut self, request: &ApprovalRequest) -> ApprovalDecision {
        // 显示审批提示
        println!();
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("⚠️  需要审批");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("操作类型: {:?}", request.operation_type);
        println!("风险等级: {}", request.risk_level);

        if let Some(reason) = &request.risk_reason {
            println!("风险原因: {}", reason);
        }

        println!();
        println!("操作内容:");
        println!("  {}", request.operation);
        println!();
        println!("选项:");
        println!("  [y] 允许执行");
        println!("  [n] 拒绝执行");
        println!("  [a] 允许并添加到白名单");
        println!("  [d] 查看详情");
        println!();

        loop {
            print!("请选择 [y/n/a/d]: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().lock().read_line(&mut input).unwrap();
            let choice = input.trim().to_lowercase();

            match choice.as_str() {
                "y" | "yes" => {
                    println!("✓ 已允许");
                    return ApprovalDecision::Allow;
                }
                "n" | "no" => {
                    println!("✗ 已拒绝");
                    return ApprovalDecision::Deny;
                }
                "a" | "always" => {
                    self.add_to_whitelist(request.operation.clone());
                    println!("✓ 已允许并添加到白名单");
                    return ApprovalDecision::AllowAlways(request.operation.clone());
                }
                "d" | "detail" => {
                    self.show_details(request);
                }
                _ => {
                    println!("无效选择，请重试");
                }
            }
        }
    }

    /// 显示详细信息
    fn show_details(&self, request: &ApprovalRequest) {
        println!();
        println!("── 详细信息 ──");
        println!("操作: {}", request.operation);
        println!("类型: {:?}", request.operation_type);
        println!("风险: {} ({:?})", request.risk_level, request.risk_level);

        if let Some(reason) = &request.risk_reason {
            println!("原因: {}", reason);
        }

        match request.risk_level {
            RiskLevel::Critical => {
                println!();
                println!("⚠️  这是高风险操作，可能导致：");
                println!("  • 数据永久丢失");
                println!("  • 系统不稳定或崩溃");
                println!("  • 安全漏洞");
            }
            RiskLevel::High => {
                println!();
                println!("⚠️  这是高风险操作，请谨慎确认。");
            }
            RiskLevel::Medium => {
                println!();
                println!("ℹ️  这是中等风险操作，建议检查后执行。");
            }
            _ => {}
        }
        println!();
    }
}

impl Default for InteractiveApprovalHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalHandler for InteractiveApprovalHandler {
    fn request_approval(&self, request: &ApprovalRequest) -> ApprovalDecision {
        // YOLO 模式直接允许
        if self.yolo_mode {
            return ApprovalDecision::Allow;
        }

        // 检查白名单
        if self.is_in_whitelist(&request.operation) {
            return ApprovalDecision::Allow;
        }

        // 需要克隆 self 来进行交互式审批
        // 这里使用内部可变性模式
        let mut this = self.clone();
        this.interactive_approval(request)
    }
}

impl Clone for InteractiveApprovalHandler {
    fn clone(&self) -> Self {
        Self {
            whitelist: self.whitelist.clone(),
            whitelist_path: self.whitelist_path.clone(),
            yolo_mode: self.yolo_mode,
        }
    }
}

/// 静默审批处理器（用于测试或自动化场景）
pub struct SilentApprovalHandler {
    /// 默认决策
    default_decision: ApprovalDecision,
}

impl SilentApprovalHandler {
    /// 创建总是允许的处理器
    pub fn always_allow() -> Self {
        Self {
            default_decision: ApprovalDecision::Allow,
        }
    }

    /// 创建总是拒绝的处理器
    pub fn always_deny() -> Self {
        Self {
            default_decision: ApprovalDecision::Deny,
        }
    }
}

impl ApprovalHandler for SilentApprovalHandler {
    fn request_approval(&self, _request: &ApprovalRequest) -> ApprovalDecision {
        self.default_decision.clone()
    }
}

/// 审批管理器
///
/// 统一管理命令和路径的审批流程
pub struct ApprovalManager {
    /// 审批处理器
    handler: Arc<dyn ApprovalHandler>,
}

impl ApprovalManager {
    /// 创建新的审批管理器
    pub fn new(handler: Arc<dyn ApprovalHandler>) -> Self {
        Self { handler }
    }

    /// 创建默认的交互式审批管理器
    pub fn interactive() -> Self {
        Self::new(Arc::new(InteractiveApprovalHandler::new()))
    }

    /// 创建静默允许的审批管理器（YOLO 模式）
    pub fn yolo() -> Self {
        Self::new(Arc::new(SilentApprovalHandler::always_allow()))
    }

    /// 请求命令审批
    pub fn approve_command(&self, command: &str, risk_info: &RiskInfo) -> ApprovalDecision {
        let request = ApprovalRequest {
            operation: command.to_string(),
            risk_level: risk_info.level,
            risk_reason: risk_info.reason.clone(),
            operation_type: OperationType::ShellCommand,
        };

        self.handler.request_approval(&request)
    }

    /// 请求文件写入审批
    pub fn approve_file_write(&self, path: &str, reason: Option<&str>) -> ApprovalDecision {
        let request = ApprovalRequest {
            operation: format!("写入文件: {}", path),
            risk_level: RiskLevel::Low,
            risk_reason: reason.map(|s| s.to_string()),
            operation_type: OperationType::FileWrite,
        };

        self.handler.request_approval(&request)
    }

    /// 请求文件删除审批
    pub fn approve_file_delete(&self, path: &str, reason: Option<&str>) -> ApprovalDecision {
        let request = ApprovalRequest {
            operation: format!("删除文件: {}", path),
            risk_level: RiskLevel::Medium,
            risk_reason: reason.map(|s| s.to_string()),
            operation_type: OperationType::FileDelete,
        };

        self.handler.request_approval(&request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silent_handler_allow() {
        let handler = SilentApprovalHandler::always_allow();
        let request = ApprovalRequest {
            operation: "rm -rf /".to_string(),
            risk_level: RiskLevel::Critical,
            risk_reason: Some("删除根目录".to_string()),
            operation_type: OperationType::ShellCommand,
        };

        assert_eq!(handler.request_approval(&request), ApprovalDecision::Allow);
    }

    #[test]
    fn test_silent_handler_deny() {
        let handler = SilentApprovalHandler::always_deny();
        let request = ApprovalRequest {
            operation: "ls".to_string(),
            risk_level: RiskLevel::Safe,
            risk_reason: None,
            operation_type: OperationType::ShellCommand,
        };

        assert_eq!(handler.request_approval(&request), ApprovalDecision::Deny);
    }

    #[test]
    fn test_approval_manager() {
        let manager = ApprovalManager::yolo();

        let risk_info = RiskInfo {
            command: "rm -rf /".to_string(),
            level: RiskLevel::Critical,
            reason: Some("删除根目录".to_string()),
            requires_approval: true,
        };

        assert_eq!(
            manager.approve_command("rm -rf /", &risk_info),
            ApprovalDecision::Allow
        );
    }
}
