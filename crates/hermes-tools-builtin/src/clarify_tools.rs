//! clarify_tools — 用户交互工具
//!
//! 支持多选一和开放式问题，回调函数由平台层注入。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// 最大选项数
const MAX_CHOICES: usize = 4;

/// 用户交互回调类型
pub type AskUserFn = Box<dyn Fn(String, Option<Vec<String>>) -> String + Send + Sync>;

/// ClarifyTool — 用户交互工具
///
/// 通过回调函数向用户提问，支持多选一和开放式问题。
pub struct ClarifyTool {
    ask_user: Arc<AskUserFn>,
}

impl ClarifyTool {
    /// 使用提供的回调创建
    pub fn new(ask_user: AskUserFn) -> Self {
        Self {
            ask_user: Arc::new(ask_user),
        }
    }

    /// 创建无回调版本（返回友好错误）
    pub fn new_noop() -> Self {
        Self::new(Box::new(|_, _| String::new()))
    }

    /// 同步执行（供测试用）
    pub fn execute_sync(&self, args: serde_json::Value) -> Result<String, ToolError> {
        #[derive(Debug, Deserialize)]
        struct ClarifyParams {
            question: Option<String>,
            #[serde(default)]
            choices: Option<Vec<String>>,
        }

        let params: ClarifyParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let question = params.question.unwrap_or_default();
        if question.trim().is_empty() {
            return Err(ToolError::InvalidArgs(
                "question text is required".into(),
            ));
        }

        let mut choices = params.choices;
        if let Some(ref c) = choices {
            if c.len() > MAX_CHOICES {
                choices = Some(c[..MAX_CHOICES].to_vec());
            }
        }

        // 检查回调是否为空（noop）
        let is_noop = (self.ask_user)(question.clone(), choices.clone()).is_empty();

        if is_noop {
            return Ok(json!({
                "error": "Clarify tool is not available in this execution context."
            })
            .to_string());
        }

        let user_response = (self.ask_user)(question.clone(), choices.clone());
        Ok(json!({
            "question": question,
            "choices_offered": choices,
            "user_response": user_response
        })
        .to_string())
    }
}

impl Clone for ClarifyTool {
    fn clone(&self) -> Self {
        Self {
            ask_user: Arc::clone(&self.ask_user),
        }
    }
}

impl std::fmt::Debug for ClarifyTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClarifyTool").finish()
    }
}

#[async_trait]
impl Tool for ClarifyTool {
    fn name(&self) -> &str {
        "clarify"
    }

    fn description(&self) -> &str {
        "Ask the user a question when you need clarification or feedback."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to present to the user."
                },
                "choices": {
                    "type": "array",
                    "items": { "type": "string" },
                    "maxItems": 4,
                    "description": "Up to 4 answer choices. Omit for open-ended question."
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _context: ToolContext,
    ) -> Result<String, ToolError> {
        self.execute_sync(args)
    }
}
