//! SecurityScanner — Skills 安全扫描器
//!
//! 检测 Skill 内容中的潜在安全威胁，包括代码执行、敏感操作等危险模式。

use regex::Regex;
use serde::{Deserialize, Serialize};

/// 威胁检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Threat {
    /// 匹配的危险模式名称
    pub pattern: String,
    /// 威胁所在行号
    pub line_number: usize,
    /// 威胁严重程度
    pub severity: Severity,
}

/// 威胁严重程度枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    /// 高危：可能导致代码执行
    High,
    /// 中危：可能导致敏感信息泄露
    Medium,
    /// 低危：潜在风险
    Low,
}

/// 扫描结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// 是否安全（无威胁）
    pub safe: bool,
    /// 检测到的威胁列表
    pub threats: Vec<Threat>,
    /// 已扫描内容数量
    #[serde(default)]
    pub scanned_count: usize,
}

/// 扫描内容，检测潜在安全威胁
///
/// # 参数
/// - `content`: 待扫描的文本内容
///
/// # 返回
/// 返回扫描结果，包含是否安全及检测到的威胁列表
pub fn scan_content(content: &str) -> ScanResult {
    let patterns: Vec<(&str, &str, Severity)> = vec![
        (r"eval\s*\(", "eval() code execution", Severity::High),
        (r"exec\s*\(", "exec() code execution", Severity::High),
        (r"compile\s*\(", "compile() code generation", Severity::High),
        (r"subprocess", "subprocess command execution", Severity::High),
        (r"os\.system", "os.system shell execution", Severity::High),
        (r"os\.popen", "os.popen shell execution", Severity::High),
        (r"__import__", "__import__ dynamic import", Severity::High),
        (r"importlib", "importlib dynamic import", Severity::High),
        (r"open\s*=\s*", "open function override", Severity::Medium),
        (r"_builtin_\.open", "builtin open override", Severity::Medium),
        (r"os\.environ\[", "environment variable access", Severity::Medium),
        (r"getenv\s*\(", "environment variable read", Severity::Medium),
        (r"\|\s*sh", "shell pipe", Severity::High),
        (r"/bin/sh", "shell execution", Severity::High),
    ];

    let mut threats = Vec::new();
    for (line_number, line) in content.lines().enumerate() {
        for (pattern, description, severity) in &patterns {
            if Regex::new(pattern).map(|re| re.is_match(line)).unwrap_or(false) {
                threats.push(Threat {
                    pattern: description.to_string(),
                    line_number: line_number + 1,
                    severity: severity.clone(),
                });
            }
        }
    }
    ScanResult {
        safe: threats.is_empty(),
        threats,
        scanned_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_detects_eval() {
        let content = "let x = eval('2 + 2');";
        let result = scan_content(content);
        assert!(!result.safe);
        assert_eq!(result.threats.len(), 1);
        assert_eq!(result.threats[0].severity, Severity::High);
    }

    #[test]
    fn test_scan_safe_content() {
        let content = "# This is a safe skill\nprint('hello')";
        let result = scan_content(content);
        assert!(result.safe);
        assert!(result.threats.is_empty());
    }

    #[test]
    fn test_scan_multiple_threats() {
        let content = "eval('x')\nsubprocess.call(['ls'])";
        let result = scan_content(content);
        assert!(!result.safe);
        assert_eq!(result.threats.len(), 2);
    }
}
