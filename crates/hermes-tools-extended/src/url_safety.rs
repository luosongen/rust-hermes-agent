//! UrlSafetyTool — URL 安全检查工具
//!
//! 检测可疑 URL，防止访问恶意或内部网络地址：
//! - IP 地址 URL（绕过域名过滤）
//! - localhost / 内网地址
//! - 已知可疑顶级域名
//! - 过长的子域名（DNS 隧道特征）

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::IpAddr;
use std::str::FromStr;

// =============================================================================
// Regex Patterns
// =============================================================================

static URL_EXTRACTOR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://[^\s'<>]+").expect("valid regex")
});

static SUSPICIOUS_TLDS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        ".tk", ".ml", ".ga", ".cf", ".gq",  // 免费域名，常被滥用
        ".top", ".xyz", ".club", ".online",  // 便宜域名
        ".pw", ".ws", ".bid", ".download",
        ".racing", ".win", ".date", ".party",
        ".click", ".link", ".work", ".men",
    ]
});

// =============================================================================
// Risk Level
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "safe",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }

    pub fn score(&self) -> u32 {
        match self {
            RiskLevel::Safe => 0,
            RiskLevel::Low => 1,
            RiskLevel::Medium => 2,
            RiskLevel::High => 3,
            RiskLevel::Critical => 4,
        }
    }
}

// =============================================================================
// URL Check Result
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlFinding {
    pub url: String,
    pub risk_level: String,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlCheckResult {
    pub overall_risk: String,
    pub risk_score: u32,
    pub urls_checked: usize,
    pub findings: Vec<UrlFinding>,
    pub safe: bool,
}

// =============================================================================
// URL Safety Checker
// =============================================================================

#[derive(Clone)]
pub struct UrlSafetyChecker;

impl UrlSafetyChecker {
    pub fn new() -> Self {
        Self
    }

    /// 从文本中提取所有 URL
    pub fn extract_urls(&self, text: &str) -> Vec<String> {
        URL_EXTRACTOR
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    /// 检查单个 URL 的安全性
    pub fn check_url(&self, url: &str) -> UrlFinding {
        let lower = url.to_lowercase();

        // 尝试解析 URL
        let parsed = match url::Url::parse(url) {
            Ok(u) => u,
            Err(_) => {
                return UrlFinding {
                    url: url.to_string(),
                    risk_level: RiskLevel::Medium.as_str().to_string(),
                    category: "malformed_url".to_string(),
                    description: "URL cannot be parsed".to_string(),
                };
            }
        };

        let host = parsed.host_str().unwrap_or("").to_lowercase();

        // 1. localhost
        if host == "localhost" || host == "127.0.0.1" || host == "::1" {
            return UrlFinding {
                url: url.to_string(),
                risk_level: RiskLevel::High.as_str().to_string(),
                category: "localhost".to_string(),
                description: "Points to localhost — potential SSRF attack".to_string(),
            };
        }

        // 2. IP 地址
        if let Ok(ip) = IpAddr::from_str(&host) {
            return self.check_ip_url(url, ip);
        }

        // 3. 内网域名
        if self.is_internal_domain(&host) {
            return UrlFinding {
                url: url.to_string(),
                risk_level: RiskLevel::High.as_str().to_string(),
                category: "internal_network".to_string(),
                description: "Points to internal network address".to_string(),
            };
        }

        // 4. 可疑 TLD
        for tld in SUSPICIOUS_TLDS.iter() {
            if host.ends_with(tld) {
                return UrlFinding {
                    url: url.to_string(),
                    risk_level: RiskLevel::Medium.as_str().to_string(),
                    category: "suspicious_tld".to_string(),
                    description: format!("Uses suspicious TLD: {}", tld),
                };
            }
        }

        // 5. 过长子域名（DNS 隧道特征）
        if host.len() > 80 {
            return UrlFinding {
                url: url.to_string(),
                risk_level: RiskLevel::Low.as_str().to_string(),
                category: "long_subdomain".to_string(),
                description: "Unusually long subdomain — possible DNS tunneling".to_string(),
            };
        }

        // 6. URL 编码混淆
        if lower.contains("%00") || lower.contains("%0a") || lower.contains("%0d") {
            return UrlFinding {
                url: url.to_string(),
                risk_level: RiskLevel::Medium.as_str().to_string(),
                category: "encoding_obfuscation".to_string(),
                description: "Contains suspicious URL-encoded characters".to_string(),
            };
        }

        // 7. 已知钓鱼关键词
        if self.is_phishing_keyword(&host) {
            return UrlFinding {
                url: url.to_string(),
                risk_level: RiskLevel::High.as_str().to_string(),
                category: "phishing_keyword".to_string(),
                description: "Contains known phishing-related keywords".to_string(),
            };
        }

        // 默认安全
        UrlFinding {
            url: url.to_string(),
            risk_level: RiskLevel::Safe.as_str().to_string(),
            category: "safe".to_string(),
            description: "No known risks detected".to_string(),
        }
    }

    fn check_ip_url(&self, url: &str, ip: IpAddr) -> UrlFinding {
        match ip {
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                // 私有地址
                if octets[0] == 10
                    || (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31)
                    || (octets[0] == 192 && octets[1] == 168)
                    || (octets[0] == 127)
                    || (octets[0] == 169 && octets[1] == 254)
                    || (octets[0] == 0)
                {
                    UrlFinding {
                        url: url.to_string(),
                        risk_level: RiskLevel::Critical.as_str().to_string(),
                        category: "private_ip".to_string(),
                        description: format!("Points to private IP: {} — SSRF risk", v4),
                    }
                } else {
                    UrlFinding {
                        url: url.to_string(),
                        risk_level: RiskLevel::Medium.as_str().to_string(),
                        category: "raw_ip".to_string(),
                        description: "Uses raw IP address instead of domain name".to_string(),
                    }
                }
            }
            IpAddr::V6(v6) => {
                if v6.is_loopback() || v6.is_unique_local() || v6.is_unspecified() {
                    UrlFinding {
                        url: url.to_string(),
                        risk_level: RiskLevel::Critical.as_str().to_string(),
                        category: "private_ipv6".to_string(),
                        description: format!("Points to private IPv6: {}", v6),
                    }
                } else {
                    UrlFinding {
                        url: url.to_string(),
                        risk_level: RiskLevel::Medium.as_str().to_string(),
                        category: "raw_ipv6".to_string(),
                        description: "Uses raw IPv6 address".to_string(),
                    }
                }
            }
        }
    }

    fn is_internal_domain(&self, host: &str) -> bool {
        let internal_suffixes = [
            ".local", ".internal", ".intranet", ".lan", ".home",
            ".corp", ".private", ".svc", ".cluster.local",
        ];
        internal_suffixes.iter().any(|suffix| host.ends_with(suffix))
    }

    fn is_phishing_keyword(&self, host: &str) -> bool {
        let keywords = [
            "login-", "signin-", "verify-", "secure-", "account-",
            "update-", "confirm-", "banking-", "password-",
        ];
        keywords.iter().any(|kw| host.contains(kw))
    }

    /// 批量检查文本中的所有 URL
    pub fn check_text(&self, text: &str) -> UrlCheckResult {
        let urls = self.extract_urls(text);
        let mut findings = Vec::new();
        let mut max_score = 0u32;

        for url in &urls {
            let finding = self.check_url(url);
            let score = match finding.risk_level.as_str() {
                "critical" => 4,
                "high" => 3,
                "medium" => 2,
                "low" => 1,
                _ => 0,
            };
            max_score = max_score.max(score);
            if finding.risk_level != "safe" {
                findings.push(finding);
            }
        }

        let overall = match max_score {
            0 => RiskLevel::Safe,
            1 => RiskLevel::Low,
            2 => RiskLevel::Medium,
            3 => RiskLevel::High,
            _ => RiskLevel::Critical,
        };

        UrlCheckResult {
            overall_risk: overall.as_str().to_string(),
            risk_score: max_score,
            urls_checked: urls.len(),
            findings,
            safe: max_score == 0,
        }
    }
}

impl Default for UrlSafetyChecker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tool Interface
// =============================================================================

#[derive(Clone)]
pub struct UrlSafetyTool {
    checker: UrlSafetyChecker,
}

impl UrlSafetyTool {
    pub fn new() -> Self {
        Self {
            checker: UrlSafetyChecker::new(),
        }
    }
}

impl Default for UrlSafetyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
pub struct UrlCheckParams {
    pub url: Option<String>,
    pub text: Option<String>,
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_action() -> String { "check".to_string() }

#[async_trait]
impl Tool for UrlSafetyTool {
    fn name(&self) -> &str {
        "url_check"
    }

    fn description(&self) -> &str {
        "Check URLs for safety risks: private IPs, localhost, suspicious TLDs, \
         phishing keywords, DNS tunneling signs. Returns risk level and findings."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Single URL to check"
                },
                "text": {
                    "type": "string",
                    "description": "Text containing URLs to extract and check"
                },
                "action": {
                    "type": "string",
                    "enum": ["check", "extract"],
                    "default": "check",
                    "description": "check: full safety report. extract: just extract URLs."
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        let params: UrlCheckParams = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        match params.action.as_str() {
            "extract" => {
                let text = params.text.ok_or_else(|| ToolError::InvalidArgs("'text' required for extract action".to_string()))?;
                let urls = self.checker.extract_urls(&text);
                Ok(json!({ "urls": urls, "count": urls.len() }).to_string())
            }
            _ => {
                if let Some(url) = params.url {
                    let finding = self.checker.check_url(&url);
                    Ok(json!({
                        "url": finding.url,
                        "risk_level": finding.risk_level,
                        "category": finding.category,
                        "description": finding.description
                    }).to_string())
                } else if let Some(text) = params.text {
                    let result = self.checker.check_text(&text);
                    let json_result = serde_json::to_string(&result)
                        .map_err(|e| ToolError::Execution(format!("JSON error: {}", e)))?;
                    Ok(json_result)
                } else {
                    Err(ToolError::InvalidArgs("Either 'url' or 'text' must be provided".to_string()))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_urls() {
        let checker = UrlSafetyChecker::new();
        let text = "Check https://example.com and http://test.org/path?a=1";
        let urls = checker.extract_urls(text);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("example.com"));
    }

    #[test]
    fn test_check_localhost() {
        let checker = UrlSafetyChecker::new();
        let result = checker.check_url("http://localhost:8080/admin");
        assert_eq!(result.risk_level, "high");
        assert_eq!(result.category, "localhost");
    }

    #[test]
    fn test_check_private_ip() {
        let checker = UrlSafetyChecker::new();
        let result = checker.check_url("http://192.168.1.1/secret");
        assert_eq!(result.risk_level, "critical");
        assert_eq!(result.category, "private_ip");
    }

    #[test]
    fn test_check_safe_url() {
        let checker = UrlSafetyChecker::new();
        let result = checker.check_url("https://github.com/NousResearch");
        assert_eq!(result.risk_level, "safe");
    }

    #[test]
    fn test_check_suspicious_tld() {
        let checker = UrlSafetyChecker::new();
        let result = checker.check_url("https://evil.xyz/malware");
        assert_eq!(result.risk_level, "medium");
        assert_eq!(result.category, "suspicious_tld");
    }

    #[test]
    fn test_check_text_multiple_urls() {
        let checker = UrlSafetyChecker::new();
        let text = "Visit https://github.com/safe and also http://192.168.1.1/bad";
        let result = checker.check_text(text);
        assert_eq!(result.urls_checked, 2);
        assert!(!result.safe);
        assert_eq!(result.findings.len(), 1);
    }

    #[test]
    fn test_check_raw_ip() {
        let checker = UrlSafetyChecker::new();
        let result = checker.check_url("http://8.8.8.8/");
        assert_eq!(result.risk_level, "medium");
        assert_eq!(result.category, "raw_ip");
    }

    #[test]
    fn test_check_phishing_keyword() {
        let checker = UrlSafetyChecker::new();
        let result = checker.check_url("https://login-secure-bank.example.com");
        assert_eq!(result.risk_level, "high");
        assert_eq!(result.category, "phishing_keyword");
    }
}
