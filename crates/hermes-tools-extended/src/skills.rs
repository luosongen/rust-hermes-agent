//! SkillsTool — Skills 管理工具
//!
//! 提供 list / view / search / sync / install / remove 操作。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;

const SKILLS_DIR: &str = ".config/hermes-agent/skills";
const MANIFEST_FILE: &str = ".bundled_manifest";
const SKILL_FILE: &str = "SKILL.md";
const SKILLS_API_URL: &str = "https://skills.sh";

#[derive(Clone)]
pub struct SkillsTool {
    skills_dir: PathBuf,
}

impl SkillsTool {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            skills_dir: PathBuf::from(home).join(SKILLS_DIR),
        }
    }
}

#[async_trait]
impl Tool for SkillsTool {
    fn name(&self) -> &str { "skills" }
    fn description(&self) -> &str {
        "Manage local and remote AI skills. Actions: list, view, search, sync, install, remove."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "oneOf": [
                {"properties": {"action": {"const": "list"}}, "required": ["action"]},
                {"properties": {"action": {"const": "view"}, "name": {"type": "string"}}, "required": ["action", "name"]},
                {"properties": {"action": {"const": "search"}, "query": {"type": "string"}, "limit": {"type": "integer"}}, "required": ["action", "query"]},
                {"properties": {"action": {"const": "sync"}}, "required": ["action"]},
                {"properties": {"action": {"const": "install"}, "name": {"type": "string"}, "source": {"type": "string"}}, "required": ["action", "name"]},
                {"properties": {"action": {"const": "remove"}, "name": {"type": "string"}}, "required": ["action", "name"]}
            ]
        })
    }
    async fn execute(&self, args: serde_json::Value, _context: ToolContext) -> Result<String, ToolError> {
        Ok(json!({"status": "ok"}).to_string())
    }
}
