use regex::Regex;
use std::time::Instant;

/// Security threat detected during scanning
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityThreat {
    pub rule_id: String,
    pub threat_type: ThreatType,
    pub severity: Severity,
    pub description: String,
    pub location: Option<String>,
}

/// Type of security threat
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum ThreatType {
    DangerousCommand,
    NetworkCall,
    FileAccess,
    EnvLeak,
    SuspiciousPattern,
}

/// Severity level of a security threat
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Result of a security scan
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityScanResult {
    pub passed: bool,
    pub threats: Vec<SecurityThreat>,
    pub scan_duration_ms: u64,
}

struct ScanRule {
    id: &'static str,
    pattern: Regex,
    threat_type: ThreatType,
    severity: Severity,
    description: &'static str,
}

pub struct SecurityScanner {
    rules: Vec<ScanRule>,
}

impl SecurityScanner {
    pub fn new() -> Self {
        let rules = vec![
            // Dangerous command rules
            ScanRule {
                id: "DANGEROUS_001",
                pattern: Regex::new(r"rm\s+-rf\s+[/~]").unwrap(),
                threat_type: ThreatType::DangerousCommand,
                severity: Severity::Critical,
                description: "Detected recursive force delete command",
            },
            ScanRule {
                id: "DANGEROUS_002",
                pattern: Regex::new(r"\bdd\s+if=").unwrap(),
                threat_type: ThreatType::DangerousCommand,
                severity: Severity::Critical,
                description: "Detected disk copy command which may overwrite data",
            },
            ScanRule {
                id: "DANGEROUS_003",
                pattern: Regex::new(r":\(\)\{\|\\:&\};").unwrap(),
                threat_type: ThreatType::DangerousCommand,
                severity: Severity::Critical,
                description: "Detected fork bomb pattern",
            },
            ScanRule {
                id: "DANGEROUS_004",
                pattern: Regex::new(r"\b(mkfs|fdisk)\b").unwrap(),
                threat_type: ThreatType::DangerousCommand,
                severity: Severity::Critical,
                description: "Detected disk formatting or partitioning command",
            },
            // Network call rules
            ScanRule {
                id: "NETWORK_001",
                pattern: Regex::new(r"\bcurl\s+http").unwrap(),
                threat_type: ThreatType::NetworkCall,
                severity: Severity::High,
                description: "Detected HTTP request via curl",
            },
            ScanRule {
                id: "NETWORK_002",
                pattern: Regex::new(r"\bwget\s+http").unwrap(),
                threat_type: ThreatType::NetworkCall,
                severity: Severity::High,
                description: "Detected HTTP request via wget",
            },
            // File access rules
            ScanRule {
                id: "FILE_001",
                pattern: Regex::new(r"/etc/passwd").unwrap(),
                threat_type: ThreatType::FileAccess,
                severity: Severity::High,
                description: "Detected attempt to access password file",
            },
            ScanRule {
                id: "FILE_002",
                pattern: Regex::new(r"~/.ssh/").unwrap(),
                threat_type: ThreatType::FileAccess,
                severity: Severity::High,
                description: "Detected attempt to access SSH directory",
            },
            // Environment leak rules
            ScanRule {
                id: "ENV_001",
                pattern: Regex::new(r"\$[A-Z_]*(API_KEY|SECRET)[A-Z_]*").unwrap(),
                threat_type: ThreatType::EnvLeak,
                severity: Severity::High,
                description: "Detected potential API_KEY or SECRET variable reference",
            },
            ScanRule {
                id: "ENV_002",
                pattern: Regex::new(r"\$[A-Z_]*(TOKEN|PASSWORD)[A-Z_]*").unwrap(),
                threat_type: ThreatType::EnvLeak,
                severity: Severity::Medium,
                description: "Detected potential TOKEN or PASSWORD variable reference",
            },
            // Additional rules to reach 12 total
            ScanRule {
                id: "SUSPICIOUS_001",
                pattern: Regex::new(r"eval\s*\(\s*\$").unwrap(),
                threat_type: ThreatType::SuspiciousPattern,
                severity: Severity::High,
                description: "Detected dynamic code execution from variable",
            },
            ScanRule {
                id: "SUSPICIOUS_002",
                pattern: Regex::new(r"base64\s+-d\s+\|").unwrap(),
                threat_type: ThreatType::SuspiciousPattern,
                severity: Severity::Medium,
                description: "Detected piped base64 decode execution",
            },
        ];
        Self { rules }
    }

    pub fn scan(&self, content: &str) -> SecurityScanResult {
        self.scan_with_force(content, false)
    }

    pub fn scan_with_force(&self, content: &str, _force: bool) -> SecurityScanResult {
        let start = Instant::now();
        let mut threats = Vec::new();

        for rule in &self.rules {
            for (line_number, line) in content.lines().enumerate() {
                if rule.pattern.is_match(line) {
                    threats.push(SecurityThreat {
                        rule_id: rule.id.to_string(),
                        threat_type: rule.threat_type.clone(),
                        severity: rule.severity.clone(),
                        description: rule.description.to_string(),
                        location: Some(format!("line {}", line_number + 1)),
                    });
                }
            }
        }

        let duration = start.elapsed();
        let scan_duration_ms = duration.as_secs() * 1000 + u64::from(duration.subsec_millis());

        SecurityScanResult {
            passed: threats.is_empty(),
            threats,
            scan_duration_ms,
        }
    }
}

impl Default for SecurityScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_dangerous_command() {
        let scanner = SecurityScanner::new();
        let result = scanner.scan("rm -rf /");
        assert!(!result.passed);
        assert_eq!(result.threats.len(), 1);
        assert_eq!(result.threats[0].rule_id, "DANGEROUS_001");
    }

    #[test]
    fn test_scan_clean_content() {
        let scanner = SecurityScanner::new();
        let result = scanner.scan("Hello, this is a safe skill.");
        assert!(result.passed);
        assert!(result.threats.is_empty());
    }

    #[test]
    fn test_scan_api_key_exposure() {
        let scanner = SecurityScanner::new();
        let result = scanner.scan("echo $MY_API_KEY is secret");
        assert!(!result.passed);
        assert!(!result.threats.is_empty());
        assert_eq!(result.threats[0].rule_id, "ENV_001");
    }
}
