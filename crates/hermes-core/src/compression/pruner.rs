use crate::{Content, Message, Role};

pub const PRUNED_TOOL_PLACEHOLDER: &str = "[Old tool output cleared to save context space]";

pub struct ToolResultPruner {
    protect_tail_tokens: Option<usize>,
    protect_tail_count: usize,
}

impl ToolResultPruner {
    pub fn new(protect_tail_count: usize, protect_tail_tokens: Option<usize>) -> Self {
        Self {
            protect_tail_tokens,
            protect_tail_count,
        }
    }

    pub fn prune(&self, messages: Vec<Message>) -> Vec<Message> {
        let mut result = Vec::with_capacity(messages.len());

        for message in messages.into_iter() {
            // 检查是否是工具结果消息：role 为 Tool 且 content 为 ToolResult
            if let Message {
                role: Role::Tool,
                content:
                    Content::ToolResult {
                        content: _,
                        tool_call_id,
                    },
                ..
            } = message
            {
                // 将旧工具结果替换为占位符
                result.push(Message {
                    role: Role::Tool,
                    content: Content::ToolResult {
                        content: PRUNED_TOOL_PLACEHOLDER.to_string(),
                        tool_call_id,
                    },
                    reasoning: None,
                    tool_call_id: None,
                    tool_name: None,
                });
            } else {
                result.push(message);
            }
        }

        result
    }
}

impl Default for ToolResultPruner {
    fn default() -> Self {
        Self::new(5, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    /// 创建 Content::ToolResult 类型消息的辅助函数
    fn tool_result_message(tool_call_id: &str, content: &str) -> Message {
        Message {
            role: Role::Tool,
            content: Content::ToolResult {
                tool_call_id: tool_call_id.to_string(),
                content: content.to_string(),
            },
            reasoning: None,
            tool_call_id: Some(tool_call_id.to_string()),
            tool_name: None,
        }
    }

    #[test]
    fn test_prune_old_tool_results() {
        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi there!"),
            tool_result_message("call_1", "Tool output here"),
        ];

        let pruner = ToolResultPruner::default();
        let pruned = pruner.prune(messages);

        assert_eq!(pruned.len(), 3);

        // 检查工具结果是否被替换为占位符
        if let Message {
            role: Role::Tool,
            content: Content::ToolResult { content, .. },
            ..
        } = &pruned[2]
        {
            assert_eq!(content, PRUNED_TOOL_PLACEHOLDER);
        } else {
            panic!("Expected ToolResult message at index 2");
        }
    }

    #[test]
    fn test_preserve_non_tool_messages() {
        let messages = vec![Message::user("Hello"), Message::assistant("Hi there!")];

        let pruner = ToolResultPruner::default();
        let pruned = pruner.prune(messages);

        assert_eq!(pruned.len(), 2);
        assert!(matches!(
            &pruned[0],
            Message {
                role: Role::User,
                ..
            }
        ));
        assert!(matches!(
            &pruned[1],
            Message {
                role: Role::Assistant,
                ..
            }
        ));
    }

    #[test]
    fn test_multiple_tool_results() {
        let messages = vec![
            Message::user("Do something"),
            Message::assistant("I'll do it"),
            tool_result_message("call_1", "Result 1"),
            tool_result_message("call_2", "Result 2"),
            tool_result_message("call_3", "Result 3"),
        ];

        let pruner = ToolResultPruner::default();
        let pruned = pruner.prune(messages);

        assert_eq!(pruned.len(), 5);

        // 所有工具结果都应被替换
        for i in 2..5 {
            if let Message {
                role: Role::Tool,
                content: Content::ToolResult { content, .. },
                ..
            } = &pruned[i]
            {
                assert_eq!(content, PRUNED_TOOL_PLACEHOLDER);
            } else {
                panic!("Expected ToolResult message at index {}", i);
            }
        }
    }
}
