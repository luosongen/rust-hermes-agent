//! BuiltinMemoryProvider - Built-in memory implementation

mod memory_store;

use async_trait::async_trait;
use std::sync::Arc;
use crate::memory_manager::MemoryProvider;
use memory_store::{MemoryStore, MemoryType};

pub struct BuiltinMemoryProvider {
    store: Arc<MemoryStore>,
}

impl BuiltinMemoryProvider {
    pub fn new(store: Arc<MemoryStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl MemoryProvider for BuiltinMemoryProvider {
    fn name(&self) -> &str {
        "builtin"
    }

    fn get_tool_schemas(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({
                "name": "memory",
                "description": "Add or remove entries from agent memory (MEMORY.md/USER.md)",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["add", "remove"],
                            "description": "Action to perform"
                        },
                        "entry": {
                            "type": "string",
                            "description": "Memory entry text"
                        },
                        "memory_type": {
                            "type": "string",
                            "enum": ["memory", "user"],
                            "description": "Type of memory (memory=MEMORY.md, user=USER.md)"
                        }
                    },
                    "required": ["action", "entry", "memory_type"]
                }
            })
        ]
    }

    fn system_prompt_block(&self) -> String {
        self.store.get_snapshot()
    }

    fn prefetch(&self, _query: &str, _session_id: &str) -> String {
        self.store.get_snapshot()
    }

    fn queue_prefetch(&self, _query: &str, _session_id: &str) {}

    fn sync_turn(&self, _user_content: &str, _assistant_content: &str, _session_id: &str) {}

    fn handle_tool_call(&self, tool_name: &str, args: serde_json::Value) -> Result<String, String> {
        if tool_name != "memory" {
            return Err(format!("Unknown tool: {}", tool_name));
        }

        let action = args.get("action").and_then(|v| v.as_str()).ok_or_else(|| "action required".to_string())?;
        let entry = args.get("entry").and_then(|v| v.as_str()).ok_or_else(|| "entry required".to_string())?;
        let memory_type_str = args.get("memory_type").and_then(|v| v.as_str()).ok_or_else(|| "memory_type required".to_string())?;

        let memory_type = match memory_type_str {
            "memory" => MemoryType::Memory,
            "user" => MemoryType::User,
            _ => return Err(format!("Invalid memory_type: {}", memory_type_str)),
        };

        match action {
            "add" => {
                self.store.add(entry, memory_type)?;
                Ok(format!("Added to {}: {}", memory_type_str, entry))
            }
            "remove" => {
                self.store.remove(entry, memory_type)?;
                Ok(format!("Removed from {}: {}", memory_type_str, entry))
            }
            _ => Err(format!("Unknown action: {}", action)),
        }
    }
}