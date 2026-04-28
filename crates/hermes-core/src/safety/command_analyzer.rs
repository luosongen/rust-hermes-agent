//! 危险命令分析器
//!
//! 检测 Shell 命令的风险等级，支持：
//! - 内置危险模式匹配
//! - 自定义规则扩展
//! - 命令上下文分析

use regex::Regex;
use std::sync::OnceLock;

/// 风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RiskLevel {
    /// 安全命令（如 ls, cat, echo）
    Safe,
    /// 低风险（可能修改用户数据，但在预期范围内）
    Low,
    /// 中风险（可能影响系统状态）
    Medium,
    /// 高风险（可能导致数据丢失或系统不稳定）
    High,
    /// 危险命令（极度危险，必须审批）
    Critical,
}

impl RiskLevel {
    /// 获取风险等级描述
    pub fn description(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "安全",
            RiskLevel::Low => "低风险",
            RiskLevel::Medium => "中风险",
            RiskLevel::High => "高风险",
            RiskLevel::Critical => "危险",
        }
    }

    /// 是否需要审批
    pub fn requires_approval(&self) -> bool {
        matches!(self, RiskLevel::Medium | RiskLevel::High | RiskLevel::Critical)
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// 危险命令模式定义
struct DangerousPattern {
    pattern: Regex,
    level: RiskLevel,
    description: &'static str,
}

/// 获取内置危险模式列表
fn get_dangerous_patterns() -> &'static Vec<DangerousPattern> {
    static PATTERNS: OnceLock<Vec<DangerousPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // Critical - 系统破坏性命令
            DangerousPattern {
                pattern: Regex::new(r"rm\s+(-[rf]+\s+)+/").unwrap(),
                level: RiskLevel::Critical,
                description: "删除根目录",
            },
            DangerousPattern {
                pattern: Regex::new(r"rm\s+(-[rf]+\s+)+~").unwrap(),
                level: RiskLevel::Critical,
                description: "删除用户主目录",
            },
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+rm\s+(-[rf]+\s+)+/").unwrap(),
                level: RiskLevel::Critical,
                description: "sudo 删除根目录",
            },
            DangerousPattern {
                pattern: Regex::new(r"mkfs").unwrap(),
                level: RiskLevel::Critical,
                description: "格式化磁盘",
            },
            DangerousPattern {
                pattern: Regex::new(r"dd\s+if=.*of=/dev/").unwrap(),
                level: RiskLevel::Critical,
                description: "dd 写入设备",
            },
            DangerousPattern {
                pattern: Regex::new(r">\s*/dev/sd[a-z]").unwrap(),
                level: RiskLevel::Critical,
                description: "写入块设备",
            },
            DangerousPattern {
                pattern: Regex::new(r":\(\)\s*\{\s*:\|:&\s*\}\s*;").unwrap(),
                level: RiskLevel::Critical,
                description: "Fork 炸弹",
            },

            // High - 数据破坏性命令
            DangerousPattern {
                pattern: Regex::new(r"rm\s+-rf").unwrap(),
                level: RiskLevel::High,
                description: "递归强制删除",
            },
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+rm").unwrap(),
                level: RiskLevel::High,
                description: "sudo 删除文件",
            },
            DangerousPattern {
                pattern: Regex::new(r"chmod\s+(-R\s+)?777").unwrap(),
                level: RiskLevel::High,
                description: "开放所有权限",
            },
            DangerousPattern {
                pattern: Regex::new(r"chown\s+(-R\s+)?.*:.*\s+/").unwrap(),
                level: RiskLevel::High,
                description: "更改根目录所有者",
            },

            // Medium - 远程执行和网络
            DangerousPattern {
                pattern: Regex::new(r"curl.*\|\s*(bash|sh|zsh)").unwrap(),
                level: RiskLevel::Medium,
                description: "从网络执行脚本",
            },
            DangerousPattern {
                pattern: Regex::new(r"wget.*\|\s*(bash|sh|zsh)").unwrap(),
                level: RiskLevel::Medium,
                description: "从网络执行脚本",
            },
            DangerousPattern {
                pattern: Regex::new(r"eval\s+.*\$\(.*\)").unwrap(),
                level: RiskLevel::Medium,
                description: "动态执行命令",
            },

            // Low - 系统修改
            DangerousPattern {
                pattern: Regex::new(r"sudo\s+").unwrap(),
                level: RiskLevel::Low,
                description: "使用 sudo",
            },
            DangerousPattern {
                pattern: Regex::new(r"apt\s+(install|remove|purge)").unwrap(),
                level: RiskLevel::Low,
                description: "包管理操作",
            },
            DangerousPattern {
                pattern: Regex::new(r"brew\s+(install|uninstall)").unwrap(),
                level: RiskLevel::Low,
                description: "Homebrew 操作",
            },
        ]
    })
}

/// 安全命令白名单
const SAFE_COMMANDS: &[&str] = &[
    "ls", "dir", "cat", "head", "tail", "less", "more", "wc", "echo", "printf",
    "pwd", "whoami", "hostname", "date", "uptime", "uname", "id", "groups",
    "grep", "find", "which", "type", "file", "stat", "tree", "du", "df",
    "git", "gh", "cargo", "rustc", "rustup", "npm", "node", "python", "python3",
    "make", "cmake", "gcc", "clang", "rust-analyzer",
];

/// 命令分析器
pub struct CommandAnalyzer {
    /// 自定义危险模式
    custom_patterns: Vec<DangerousPattern>,
}

impl Default for CommandAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandAnalyzer {
    /// 创建新的命令分析器
    pub fn new() -> Self {
        Self {
            custom_patterns: Vec::new(),
        }
    }

    /// 添加自定义危险模式
    pub fn add_pattern(&mut self, pattern: Regex, level: RiskLevel, description: &'static str) {
        self.custom_patterns.push(DangerousPattern {
            pattern,
            level,
            description,
        });
    }

    /// 分析命令的风险等级
    pub fn analyze(&self, command: &str) -> RiskLevel {
        let command = command.trim();

        // 空命令安全
        if command.is_empty() {
            return RiskLevel::Safe;
        }

        // 获取命令基础名称
        let base_cmd = command.split_whitespace().next().unwrap_or("");

        // 危险字符检测 - 命令链和替换
        let dangerous_chars = ['|', '$', '`', ';', '&', '<', '>', '\n'];
        let has_dangerous_chars = dangerous_chars.iter().any(|c| command.contains(*c));

        // 检查白名单
        if SAFE_COMMANDS.contains(&base_cmd) && !has_dangerous_chars {
            return RiskLevel::Safe;
        }

        // 检查自定义模式
        for pattern in &self.custom_patterns {
            if pattern.pattern.is_match(command) {
                return pattern.level;
            }
        }

        // 检查内置危险模式
        for pattern in get_dangerous_patterns() {
            if pattern.pattern.is_match(command) {
                return pattern.level;
            }
        }

        // 包含危险字符提升风险等级
        if has_dangerous_chars {
            return RiskLevel::Medium;
        }

        // 默认低风险
        RiskLevel::Low
    }

    /// 获取命令的风险原因
    pub fn get_risk_reason(&self, command: &str) -> Option<String> {
        let command = command.trim();

        // 检查自定义模式
        for pattern in &self.custom_patterns {
            if pattern.pattern.is_match(command) {
                return Some(pattern.description.to_string());
            }
        }

        // 检查内置危险模式
        for pattern in get_dangerous_patterns() {
            if pattern.pattern.is_match(command) {
                return Some(pattern.description.to_string());
            }
        }

        None
    }

    /// 获取详细的风险信息
    pub fn get_risk_info(&self, command: &str) -> RiskInfo {
        let level = self.analyze(command);
        let reason = self.get_risk_reason(command);

        RiskInfo {
            command: command.to_string(),
            level,
            reason,
            requires_approval: level.requires_approval(),
        }
    }
}

/// 命令风险信息
#[derive(Debug, Clone)]
pub struct RiskInfo {
    /// 原始命令
    pub command: String,
    /// 风险等级
    pub level: RiskLevel,
    /// 风险原因
    pub reason: Option<String>,
    /// 是否需要审批
    pub requires_approval: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands() {
        let analyzer = CommandAnalyzer::new();

        assert_eq!(analyzer.analyze("ls -la"), RiskLevel::Safe);
        assert_eq!(analyzer.analyze("cat file.txt"), RiskLevel::Safe);
        assert_eq!(analyzer.analyze("echo hello"), RiskLevel::Safe);
        assert_eq!(analyzer.analyze("git status"), RiskLevel::Safe);
    }

    #[test]
    fn test_dangerous_commands() {
        let analyzer = CommandAnalyzer::new();

        assert_eq!(analyzer.analyze("rm -rf /"), RiskLevel::Critical);
        assert_eq!(analyzer.analyze("rm -rf ~"), RiskLevel::Critical);
        assert_eq!(analyzer.analyze("mkfs /dev/sda1"), RiskLevel::Critical);
        assert_eq!(analyzer.analyze("rm -rf ./build"), RiskLevel::High);
        assert_eq!(analyzer.analyze("sudo rm file.txt"), RiskLevel::High);
        assert_eq!(analyzer.analyze("chmod 777 file"), RiskLevel::High);
        assert_eq!(analyzer.analyze("curl https://example.com | bash"), RiskLevel::Medium);
        assert_eq!(analyzer.analyze("sudo apt install foo"), RiskLevel::Low);
    }

    #[test]
    fn test_risk_reason() {
        let analyzer = CommandAnalyzer::new();

        let info = analyzer.get_risk_info("rm -rf /");
        assert_eq!(info.level, RiskLevel::Critical);
        assert!(info.reason.is_some());
        assert!(info.requires_approval);
    }
}
