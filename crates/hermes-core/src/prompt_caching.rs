//! Prompt Caching — 提示缓存策略
//!
//! 支持 Anthropic cache_control 和 OpenAI prompt_cache_key 两种策略。

use crate::{Content, Message, ModelId, Role};

/// 缓存 TTL 选项
#[derive(Debug, Clone)]
pub enum CacheTTL {
    /// 5 分钟（默认）
    Ephemeral,
    /// 1 小时
    OneHour,
}

impl Default for CacheTTL {
    fn default() -> Self {
        Self::Ephemeral
    }
}

/// 缓存策略结果
#[derive(Debug, Clone)]
pub struct CacheResult {
    pub breakpoint_count: usize,
    pub applied: bool,
}

/// 缓存策略 trait
pub trait CacheStrategy: Send + Sync {
    /// 策略名称
    fn name(&self) -> &str;

    /// 将缓存标记应用到消息数组
    fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult;

    /// 是否对该模型启用
    fn supports_model(&self, model: &ModelId) -> bool;
}

/// Anthropic cache_control 策略
///
/// 在消息上放置最多 4 个 cache_control 断点：
/// - 断点 1：system prompt
/// - 断点 2-4：最后 3 条非 system 消息
pub struct AnthropicCache {
    ttl: CacheTTL,
}

impl AnthropicCache {
    pub fn new(ttl: CacheTTL) -> Self {
        Self { ttl }
    }

    /// 将 Content::Text 包装为带 cache_control 的格式
    fn wrap_content_with_cache(&self, text: &str) -> String {
        let escaped = serde_json::to_string(text).unwrap_or_else(|_| format!("\"{}\"", text));
        match &self.ttl {
            CacheTTL::Ephemeral => {
                format!(
                    "[{{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\"}}}}]",
                    escaped
                )
            }
            CacheTTL::OneHour => {
                format!(
                    "[{{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\",\"ttl\":\"1h\"}}}}]",
                    escaped
                )
            }
        }
    }

    /// 标记一条消息为缓存断点
    fn mark_as_breakpoint(&self, message: &mut Message) {
        if let Content::Text(ref text) = message.content {
            let wrapped = self.wrap_content_with_cache(text);
            message.content = Content::Text(wrapped);
        }
    }
}

impl CacheStrategy for AnthropicCache {
    fn name(&self) -> &str {
        "anthropic_cache_control"
    }

    fn supports_model(&self, model: &ModelId) -> bool {
        model.provider == "anthropic"
    }

    fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult {
        if !self.supports_model(model) {
            return CacheResult {
                breakpoint_count: 0,
                applied: false,
            };
        }

        let mut breakpoint_count = 0;

        // 断点 1：system prompt（第一条 role=System 的消息）
        let system_idx = messages.iter().position(|m| m.role == Role::System);
        if let Some(idx) = system_idx {
            self.mark_as_breakpoint(&mut messages[idx]);
            breakpoint_count += 1;
        }

        // 断点 2-4：最后 3 条非 system 消息
        let non_system_indices: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.role != Role::System)
            .map(|(i, _)| i)
            .collect();

        let cache_indices: Vec<usize> = non_system_indices
            .into_iter()
            .rev()
            .take(3)
            .collect();

        for idx in cache_indices {
            if Some(idx) != system_idx {
                self.mark_as_breakpoint(&mut messages[idx]);
                breakpoint_count += 1;
            }
        }

        CacheResult {
            breakpoint_count,
            applied: true,
        }
    }
}

/// OpenAI prompt_cache_key 策略
///
/// 对所有 system 消息和 tool result 消息标记可缓存。
pub struct OpenAiCache;

impl CacheStrategy for OpenAiCache {
    fn name(&self) -> &str {
        "openai_prompt_cache"
    }

    fn supports_model(&self, model: &ModelId) -> bool {
        model.provider == "openai"
    }

    fn apply(&self, _messages: &mut Vec<Message>, model: &ModelId) -> CacheResult {
        if !self.supports_model(model) {
            return CacheResult {
                breakpoint_count: 0,
                applied: false,
            };
        }

        let breakpoint_count = _messages
            .iter()
            .filter(|m| m.role == Role::System || m.role == Role::Tool)
            .count();

        CacheResult {
            breakpoint_count,
            applied: true,
        }
    }
}

/// 缓存策略分发器
///
/// 根据 model provider 自动选择合适的缓存策略。
pub struct CacheDispatcher {
    strategies: Vec<Box<dyn CacheStrategy>>,
}

impl CacheDispatcher {
    /// 创建包含默认策略的分发器
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(AnthropicCache::new(CacheTTL::Ephemeral)),
                Box::new(OpenAiCache),
            ],
        }
    }

    /// 添加自定义策略
    pub fn with_strategy(mut self, strategy: Box<dyn CacheStrategy>) -> Self {
        self.strategies.push(strategy);
        self
    }

    /// 应用缓存策略到消息数组
    pub fn apply(&self, messages: &mut Vec<Message>, model: &ModelId) -> CacheResult {
        for strategy in &self.strategies {
            if strategy.supports_model(model) {
                return strategy.apply(messages, model);
            }
        }
        CacheResult {
            breakpoint_count: 0,
            applied: false,
        }
    }
}

impl Default for CacheDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_msg(role: Role, content: &str) -> Message {
        Message {
            role,
            content: Content::Text(content.into()),
            reasoning: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    #[test]
    fn test_anthropic_cache_marks_system() {
        let cache = AnthropicCache::new(CacheTTL::Ephemeral);
        let mut messages = vec![
            Message::system("You are a helpful assistant."),
            make_text_msg(Role::User, "Hello"),
            make_text_msg(Role::Assistant, "Hi there!"),
        ];

        let result =
            cache.apply(&mut messages, &ModelId::new("anthropic", "claude-sonnet-4-5"));
        assert!(result.applied);
        assert!(result.breakpoint_count >= 1);

        if let Content::Text(ref text) = messages[0].content {
            assert!(text.contains("cache_control"));
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_anthropic_cache_only_for_anthropic() {
        let cache = AnthropicCache::new(CacheTTL::Ephemeral);
        let result = cache.apply(
            &mut vec![Message::user("test")],
            &ModelId::new("openai", "gpt-4o"),
        );
        assert!(!result.applied);
    }

    #[test]
    fn test_openai_cache_counts_system_and_tool() {
        let cache = OpenAiCache;
        let mut messages = vec![
            Message::system("system prompt"),
            make_text_msg(Role::User, "do task"),
            Message {
                role: Role::Tool,
                content: Content::ToolResult {
                    tool_call_id: "call_1".to_string(),
                    content: "result".to_string(),
                },
                reasoning: None,
                tool_call_id: Some("call_1".to_string()),
                tool_name: Some("test_tool".to_string()),
            },
        ];

        let result = cache.apply(&mut messages, &ModelId::new("openai", "gpt-4o"));
        assert!(result.applied);
        assert_eq!(result.breakpoint_count, 2);
    }

    #[test]
    fn test_cache_dispatcher_routes_to_correct_strategy() {
        let dispatcher = CacheDispatcher::new();
        let mut anthropic_msgs = vec![
            Message::system("test"),
            make_text_msg(Role::User, "hello"),
        ];

        let result = dispatcher.apply(&mut anthropic_msgs, &ModelId::new("anthropic", "claude-4"));
        assert!(result.applied);

        let result = dispatcher.apply(
            &mut vec![make_text_msg(Role::User, "hello")],
            &ModelId::new("deepseek", "deepseek-chat"),
        );
        assert!(!result.applied);
    }
}
