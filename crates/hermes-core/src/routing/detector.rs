//! 消息复杂度检测器
//!
//! 用于判断消息复杂度，以决定是否使用廉价模型。

use std::collections::HashSet;
use regex::Regex;

const URL_REGEX: &str = r"https?://|www\.";

pub const COMPLEX_KEYWORDS: &[&str] = &[
    "debug", "debugging", "implement", "implementation",
    "refactor", "patch", "traceback", "stacktrace",
    "exception", "error", "analyze", "analysis",
    "investigate", "architecture", "design", "compare",
    "benchmark", "optimize", "optimise", "review",
    "terminal", "shell", "tool", "tools", "pytest",
    "test", "tests", "plan", "planning", "delegate",
    "subagent", "cron", "docker", "kubernetes",
];

/// 复杂度检测结果
#[derive(Debug, Clone)]
pub struct ComplexityResult {
    pub is_simple: bool,
    pub confidence: f32,
}

/// 复杂度检测器
#[derive(Debug, Clone)]
pub struct ComplexityDetector {
    complex_keywords: HashSet<&'static str>,
    max_simple_chars: usize,
    max_simple_words: usize,
    max_newlines: usize,
    url_regex: Regex,
}

impl ComplexityDetector {
    /// 创建新的检测器
    pub fn new(
        max_simple_chars: usize,
        max_simple_words: usize,
        max_newlines: usize,
    ) -> Self {
        Self {
            complex_keywords: COMPLEX_KEYWORDS.iter().cloned().collect(),
            max_simple_chars,
            max_simple_words,
            max_newlines,
            url_regex: Regex::new(URL_REGEX).unwrap(),
        }
    }

    /// 判断消息是否简单
    pub fn is_simple(&self, text: &str) -> bool {
        let text = text.trim();

        // Length checks
        if text.len() > self.max_simple_chars {
            return false;
        }
        if text.split_whitespace().count() > self.max_simple_words {
            return false;
        }
        if text.matches('\n').count() > self.max_newlines {
            return false;
        }

        // Code check
        if text.contains("```") || text.contains('`') {
            return false;
        }

        // URL check
        if self.url_regex.is_match(text) {
            return false;
        }

        // Keyword check
        let lower_text = text.to_lowercase();
        let words: HashSet<&str> = lower_text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();

        if words.intersection(&self.complex_keywords).count() > 0 {
            return false;
        }

        true
    }

    /// 分析消息复杂度
    pub fn detect(&self, message: &str) -> ComplexityResult {
        let is_simple = self.is_simple(message);
        let confidence = if is_simple { 0.9 } else { 0.7 };

        ComplexityResult { is_simple, confidence }
    }
}

impl Default for ComplexityDetector {
    fn default() -> Self {
        Self::new(160, 28, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_message() {
        let detector = ComplexityDetector::default();
        assert!(detector.is_simple("Hello, how are you?"));
        assert!(detector.is_simple("What is the weather?"));
    }

    #[test]
    fn test_complex_message_with_keyword() {
        let detector = ComplexityDetector::default();
        assert!(!detector.is_simple("Debug this error in my code"));
        assert!(!detector.is_simple("Implement a new feature"));
    }

    #[test]
    fn test_complex_message_with_code() {
        let detector = ComplexityDetector::default();
        assert!(!detector.is_simple("Check this: `let x = 1`"));
        assert!(!detector.is_simple("```python\nprint('hello')\n```"));
    }

    #[test]
    fn test_message_with_url() {
        let detector = ComplexityDetector::default();
        assert!(!detector.is_simple("Check https://example.com"));
        assert!(!detector.is_simple("Visit www.test.com for info"));
    }

    #[test]
    fn test_message_too_long() {
        let detector = ComplexityDetector::new(10, 5, 1);
        assert!(!detector.is_simple("This is a very long message that exceeds the limit"));
    }
}
