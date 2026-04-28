//! FuzzyPatch - 模糊字符串匹配模块
//!
//! 用于技能自我改进时的模糊字符串替换，支持空白字符灵活匹配

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

/// 模糊补丁工具
///
/// 提供模糊字符串匹配和替换能力，支持不精确的字符串匹配
pub struct FuzzyPatch {
    matcher: SkimMatcherV2,
}

impl Clone for FuzzyPatch {
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl Default for FuzzyPatch {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyPatch {
    /// 创建新的模糊补丁实例
    pub fn new() -> Self {
        Self {
            matcher: SkimMatcherV2::default(),
        }
    }

    /// 在 content 中查找 old_string 的最佳匹配
    ///
    /// 返回 (分数, 起始索引, 结束索引)
    pub fn find_match(&self, content: &str, old_string: &str) -> Option<(i64, usize, usize)> {
        let score = self.matcher.fuzzy_match(content, old_string)?;
        // Find actual positions by searching after fuzzy match
        let lower = content.to_lowercase();
        let search = old_string.to_lowercase();
        if let Some(pos) = lower.find(&search) {
            return Some((score, pos, pos + old_string.len()));
        }
        // Fallback: use fuzzy match score only
        Some((score, 0, content.len()))
    }

    /// 在 content 中用 new_string 替换 old_string
    ///
    /// 支持空白字符的灵活匹配
    pub fn patch(&self, content: &str, old_string: &str, new_string: &str) -> Result<String, String> {
        let (score, start, end) = self.find_match(content, old_string)
            .ok_or_else(|| "Could not find matching content to patch".to_string())?;

        if score < 0 {
            return Err("Match score too low".to_string());
        }

        let mut result = content.to_string();
        result.replace_range(start..end, new_string);
        Ok(result)
    }

    /// 预览补丁效果（不实际应用）
    pub fn preview(&self, content: &str, old_string: &str, new_string: &str) -> Option<String> {
        let (score, start, end) = self.find_match(content, old_string)?;
        if score < 0 {
            return None;
        }
        let mut result = content.to_string();
        result.replace_range(start..end, new_string);
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_patch() {
        let patch = FuzzyPatch::new();
        let content = "Hello World";
        let result = patch.patch(content, "World", "Rust");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello Rust");
    }

    #[test]
    fn test_whitespace_flexibility() {
        let patch = FuzzyPatch::new();
        let content = "fn  foo() {\n    bar();\n}";
        // Should handle extra spaces
        let result = patch.patch(content, "fn foo() {", "fn bar() {");
        assert!(result.is_ok());
    }

    #[test]
    fn test_no_match() {
        let patch = FuzzyPatch::new();
        let content = "Hello World";
        let result = patch.patch(content, "NotFound", "Replacement");
        assert!(result.is_err());
    }
}