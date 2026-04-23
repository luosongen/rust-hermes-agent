//! FTS5 query sanitization

use regex::Regex;
use once_cell::Lazy;

static FTS_SPECIAL_CHARS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"[+\-&|!(){}[\]^"~*?:\\/]"#.into()).unwrap()
});

/// Sanitize user query for FTS5
pub fn sanitize_fts_query(query: &str) -> String {
    let mut result = query.to_string();

    result = result.trim()
        .trim_start_matches(|c: char| "+-&|!(){}[]^\"~*?:\\/".contains(c))
        .trim_end_matches(|c: char| "+-&|!(){}[]^\"~*?:\\/".contains(c))
        .to_string();

    let hyphenated = Regex::new(r"\b(\w+-\w+)\b").unwrap();
    result = hyphenated.replace_all(&result, "\"$1\"").to_string();

    let dotted = Regex::new(r"\b(\w+\.\w+)\b").unwrap();
    result = dotted.replace_all(&result, "\"$1\"").to_string();

    result = FTS_SPECIAL_CHARS.replace_all(&result, " ").to_string();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_basic() {
        assert_eq!(sanitize_fts_query("hello world"), "hello world");
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize_fts_query("hello + world"), "hello world");
    }

    #[test]
    fn test_sanitize_hyphenated() {
        assert_eq!(sanitize_fts_query("well-known"), "\"well-known\"");
    }
}