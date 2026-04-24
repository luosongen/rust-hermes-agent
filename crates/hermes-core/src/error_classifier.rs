//! Error Classifier — API 错误分类器
//!
//! 集中式错误分类管道，将 ProviderError 映射为 FailoverReason + 恢复建议。

use crate::ProviderError;

/// 故障转移原因
#[derive(Debug, Clone, PartialEq)]
pub enum FailoverReason {
    Auth,
    AuthPermanent,
    Billing,
    RateLimit,
    Overloaded,
    ServerError,
    Timeout,
    ContextOverflow,
    PayloadTooLarge,
    ModelNotFound,
    FormatError,
    ThinkingSignature,
    LongContextTier,
    Unknown,
}

/// 分类后的错误
#[derive(Debug, Clone)]
pub struct ClassifiedError {
    pub reason: FailoverReason,
    pub status_code: Option<u16>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub message: String,
    pub retryable: bool,
    pub should_compress: bool,
    pub should_rotate_credential: bool,
    pub should_fallback: bool,
}

impl ClassifiedError {
    /// 是否为认证相关错误
    pub fn is_auth(&self) -> bool {
        matches!(
            self.reason,
            FailoverReason::Auth | FailoverReason::AuthPermanent
        )
    }
}

// =============================================================================
// Pattern Constants
// =============================================================================

/// 计费相关错误模式
const BILLING_PATTERNS: &[&str] = &[
    "insufficient credits",
    "insufficient_quota",
    "credit balance",
    "credits have been exhausted",
    "top up your credits",
    "payment required",
    "billing hard limit",
    "exceeded your current quota",
    "account is deactivated",
    "plan does not include",
];

/// 速率限制错误模式
const RATE_LIMIT_PATTERNS: &[&str] = &[
    "rate limit",
    "rate_limit",
    "too many requests",
    "throttled",
    "requests per minute",
    "tokens per minute",
    "requests per day",
    "try again in",
    "please retry after",
    "resource_exhausted",
    "rate increased too quickly",
];

/// 上下文溢出错误模式
const CONTEXT_OVERFLOW_PATTERNS: &[&str] = &[
    "context length",
    "context_length_exceeded",
    "context window",
    "maximum context",
    "max context",
    "token limit",
    "reduce the length",
    "too long",
    "too many tokens",
    "beyond the maximum",
    // 中文错误信息
    "超过最大长度",
    "超出上下文",
    "上下文长度",
    "input length",
    "input is too long",
    "input too long",
    "request too large",
    "prompt too long",
    "context size",
    "reduce your prompt",
];

/// 认证错误模式
const AUTH_PATTERNS: &[&str] = &[
    "invalid api key",
    "invalid x-api-key",
    "incorrect api key",
    "invalid key",
    "unauthorized",
    "authentication",
    "not authenticated",
    "bad credentials",
    "no api key provided",
];

/// 模型未找到模式
const MODEL_NOT_FOUND_PATTERNS: &[&str] = &[
    "model not found",
    "does not exist",
    "no such model",
    "invalid model",
    "model is not available",
    "deprecated",
    "end of life",
];

/// 服务端断开模式
const SERVER_DISCONNECT_PATTERNS: &[&str] = &[
    "server disconnected",
    "connection closed",
    "connection reset",
    "eof",
    "peer closed",
    "broken pipe",
    "connection refused",
];

// =============================================================================
// Classification Logic
// =============================================================================

/// 按 HTTP 状态码分类
fn classify_by_status(
    status_code: u16,
    error_msg: &str,
    approx_tokens: usize,
    context_length: usize,
    model: Option<&str>,
) -> Option<ClassifiedError> {
    let msg = error_msg.to_lowercase();
    let make = |reason,
                retryable,
                should_compress,
                should_rotate,
                should_fallback| {
        ClassifiedError {
            reason,
            status_code: Some(status_code),
            provider: None,
            model: model.map(|s| s.to_string()),
            message: error_msg.to_string(),
            retryable,
            should_compress,
            should_rotate_credential: should_rotate,
            should_fallback,
        }
    };

    match status_code {
        401 => Some(make(
            FailoverReason::Auth,
            false,
            false,
            false,
            false,
        )),
        403 => {
            if BILLING_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(
                    FailoverReason::Billing,
                    false,
                    false,
                    false,
                    true,
                ))
            } else {
                Some(make(
                    FailoverReason::AuthPermanent,
                    false,
                    false,
                    true,
                    false,
                ))
            }
        }
        402 => {
            if RATE_LIMIT_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(
                    FailoverReason::RateLimit,
                    true,
                    false,
                    false,
                    false,
                ))
            } else {
                Some(make(
                    FailoverReason::Billing,
                    false,
                    false,
                    false,
                    true,
                ))
            }
        }
        404 => Some(make(
            FailoverReason::ModelNotFound,
            false,
            false,
            false,
            false,
        )),
        413 => Some(make(
            FailoverReason::PayloadTooLarge,
            false,
            true,
            false,
            false,
        )),
        429 => Some(make(
            FailoverReason::RateLimit,
            true,
            false,
            false,
            false,
        )),
        500..=502 => Some(make(
            FailoverReason::ServerError,
            true,
            false,
            false,
            false,
        )),
        503 | 529 => Some(make(
            FailoverReason::Overloaded,
            true,
            false,
            false,
            true,
        )),
        400 => {
            if CONTEXT_OVERFLOW_PATTERNS.iter().any(|p| msg.contains(p)) {
                Some(make(
                    FailoverReason::ContextOverflow,
                    false,
                    true,
                    false,
                    false,
                ))
            } else if msg.contains("too long") || approx_tokens > context_length {
                Some(make(
                    FailoverReason::ContextOverflow,
                    false,
                    true,
                    false,
                    false,
                ))
            } else if MODEL_NOT_FOUND_PATTERNS
                .iter()
                .any(|p| msg.contains(p))
            {
                Some(make(
                    FailoverReason::ModelNotFound,
                    false,
                    false,
                    false,
                    false,
                ))
            } else {
                Some(make(
                    FailoverReason::FormatError,
                    false,
                    false,
                    false,
                    false,
                ))
            }
        }
        _ => None,
    }
}

// =============================================================================
// Public API
// =============================================================================

/// 分类 API 错误 — 主入口
pub fn classify_api_error(
    error: &ProviderError,
    provider: Option<&str>,
    model: Option<&str>,
    approx_tokens: usize,
    context_length: usize,
) -> ClassifiedError {
    let msg = error.to_string();
    let msg_lower = msg.to_lowercase();

    // Step 1: 按 ProviderError variant 预分类
    match error {
        ProviderError::Auth => {
            return ClassifiedError {
                reason: FailoverReason::Auth,
                status_code: None,
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: false,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: false,
            };
        }
        ProviderError::RateLimit(secs) => {
            return ClassifiedError {
                reason: FailoverReason::RateLimit,
                status_code: Some(429),
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: true,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: *secs > 60,
            };
        }
        ProviderError::ContextTooLarge => {
            return ClassifiedError {
                reason: FailoverReason::ContextOverflow,
                status_code: Some(400),
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: false,
                should_compress: true,
                should_rotate_credential: false,
                should_fallback: false,
            };
        }
        ProviderError::InvalidModel(_) => {
            return ClassifiedError {
                reason: FailoverReason::ModelNotFound,
                status_code: Some(404),
                provider: provider.map(|s| s.to_string()),
                model: model.map(|s| s.to_string()),
                message: msg,
                retryable: false,
                should_compress: false,
                should_rotate_credential: false,
                should_fallback: false,
            };
        }
        ProviderError::Api(_) | ProviderError::Network(_) => {
            // 需要进一步按消息内容分类
        }
    }

    // Step 2: 按消息模式匹配
    if AUTH_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::Auth,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
    }

    if BILLING_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::Billing,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: true,
        };
    }

    if RATE_LIMIT_PATTERNS.iter().any(|p| msg_lower.contains(p)) {
        return ClassifiedError {
            reason: FailoverReason::RateLimit,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: true,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
    }

    if CONTEXT_OVERFLOW_PATTERNS
        .iter()
        .any(|p| msg_lower.contains(p))
    {
        return ClassifiedError {
            reason: FailoverReason::ContextOverflow,
            status_code: None,
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: msg,
            retryable: false,
            should_compress: true,
            should_rotate_credential: false,
            should_fallback: false,
        };
    }

    // Step 3: Transport error heuristics
    if let ProviderError::Network(_) = error {
        if SERVER_DISCONNECT_PATTERNS
            .iter()
            .any(|p| msg_lower.contains(p))
        {
            if approx_tokens > context_length / 2 {
                return ClassifiedError {
                    reason: FailoverReason::ContextOverflow,
                    status_code: None,
                    provider: provider.map(|s| s.to_string()),
                    model: model.map(|s| s.to_string()),
                    message: msg,
                    retryable: false,
                    should_compress: true,
                    should_rotate_credential: false,
                    should_fallback: false,
                };
            }
        }
    }

    // Step 4: Fallback
    let is_retryable = error.is_retryable();
    ClassifiedError {
        reason: FailoverReason::Unknown,
        status_code: None,
        provider: provider.map(|s| s.to_string()),
        model: model.map(|s| s.to_string()),
        message: msg,
        retryable: is_retryable,
        should_compress: false,
        should_rotate_credential: false,
        should_fallback: !is_retryable,
    }
}

/// 按 HTTP 状态码 + 内容分类
pub fn classify_http_error(
    status_code: u16,
    error_body: &str,
    provider: Option<&str>,
    model: Option<&str>,
    approx_tokens: usize,
    context_length: usize,
) -> ClassifiedError {
    let status_result = classify_by_status(
        status_code,
        error_body,
        approx_tokens,
        context_length,
        model,
    );

    if let Some(mut result) = status_result {
        result.provider = provider.map(|s| s.to_string());
        result
    } else {
        let (reason, retryable, should_fallback) = match status_code {
            s if s < 500 => (FailoverReason::Unknown, false, false),
            _ => (FailoverReason::ServerError, true, false),
        };

        ClassifiedError {
            reason,
            status_code: Some(status_code),
            provider: provider.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            message: error_body.to_string(),
            retryable,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_auth_error() {
        let result = classify_api_error(
            &ProviderError::Auth,
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Auth);
        assert!(!result.retryable);
        assert!(!result.should_fallback);
    }

    #[test]
    fn test_classify_rate_limit() {
        let result = classify_api_error(
            &ProviderError::RateLimit(30),
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert!(result.retryable);
        assert!(!result.should_fallback); // 30s < 60s -> no fallback
    }

    #[test]
    fn test_classify_long_rate_limit() {
        let result = classify_api_error(
            &ProviderError::RateLimit(120),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert!(result.should_fallback); // 120s > 60s -> fallback
    }

    #[test]
    fn test_classify_context_too_large() {
        let result = classify_api_error(
            &ProviderError::ContextTooLarge,
            Some("anthropic"),
            None,
            200000,
            200000,
        );
        assert_eq!(result.reason, FailoverReason::ContextOverflow);
        assert!(result.should_compress);
    }

    #[test]
    fn test_classify_invalid_model() {
        let result = classify_api_error(
            &ProviderError::InvalidModel("gpt-5".into()),
            Some("openai"),
            None,
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::ModelNotFound);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_api_billing_message() {
        let result = classify_api_error(
            &ProviderError::Api("insufficient credits balance".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Billing);
        assert!(result.should_fallback);
    }

    #[test]
    fn test_classify_api_rate_limit_message() {
        let result = classify_api_error(
            &ProviderError::Api("rate limit exceeded, too many requests".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert!(result.retryable);
    }

    #[test]
    fn test_classify_api_context_overflow_message() {
        let result = classify_api_error(
            &ProviderError::Api("context length exceeded: 200000 tokens".into()),
            Some("anthropic"),
            None,
            200000,
            200000,
        );
        assert_eq!(result.reason, FailoverReason::ContextOverflow);
        assert!(result.should_compress);
    }

    #[test]
    fn test_classify_api_auth_message() {
        let result = classify_api_error(
            &ProviderError::Api("invalid api key provided".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Auth);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_api_unknown_message() {
        let result = classify_api_error(
            &ProviderError::Api("something unexpected happened".into()),
            Some("openai"),
            Some("gpt-4o"),
            5000,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Unknown);
        assert!(result.retryable); // Api is retryable
        assert!(!result.should_fallback);
    }

    #[test]
    fn test_classified_error_is_auth() {
        let auth = ClassifiedError {
            reason: FailoverReason::Auth,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(auth.is_auth());

        let auth_permanent = ClassifiedError {
            reason: FailoverReason::AuthPermanent,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(auth_permanent.is_auth());

        let billing = ClassifiedError {
            reason: FailoverReason::Billing,
            status_code: None,
            provider: None,
            model: None,
            message: String::new(),
            retryable: false,
            should_compress: false,
            should_rotate_credential: false,
            should_fallback: false,
        };
        assert!(!billing.is_auth());
    }

    #[test]
    fn test_classify_http_error() {
        let result = classify_http_error(
            429,
            "Too many requests",
            Some("openai"),
            Some("gpt-4o"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::RateLimit);
        assert_eq!(result.status_code, Some(429));
        assert!(result.retryable);
    }

    #[test]
    fn test_classify_http_404_model_not_found() {
        let result = classify_http_error(
            404,
            "model not found",
            Some("openai"),
            Some("nonexistent-model"),
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::ModelNotFound);
        assert!(!result.retryable);
    }

    #[test]
    fn test_classify_http_503_overloaded() {
        let result = classify_http_error(
            503,
            "Service unavailable",
            Some("openai"),
            None,
            100,
            128000,
        );
        assert_eq!(result.reason, FailoverReason::Overloaded);
        assert!(result.retryable);
        assert!(result.should_fallback);
    }
}
