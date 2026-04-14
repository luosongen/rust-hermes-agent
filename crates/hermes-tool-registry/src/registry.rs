use async_trait::async_trait;
use hermes_core::{ToolContext, ToolDefinition, ToolDispatcher, ToolError};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Tool trait — all tools must implement this.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError>;
    fn precheck(&self) -> Result<(), ToolError> {
        Ok(())
    }
}

/// Manages registered tools and implements ToolDispatcher for use by Agent.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<T: Tool + 'static>(&self, tool: T) {
        let name = tool.name().to_string();
        self.tools.write().insert(name, Arc::new(tool));
    }

    pub fn unregister(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.write().remove(name)
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.read().get(name).cloned()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.read().keys().cloned().collect()
    }

    pub fn get_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .read()
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }

    pub fn check_all_preconditions(&self) -> Vec<(String, ToolError)> {
        self.tools
            .read()
            .values()
            .filter_map(|t| t.precheck().err().map(|e| (t.name().to_string(), e)))
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolDispatcher for ToolRegistry {
    fn get_definitions(&self) -> Vec<ToolDefinition> {
        self.get_tool_definitions()
    }

    async fn dispatch(
        &self,
        call: &hermes_core::ToolCall,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let tool = self
            .get(&call.name)
            .ok_or_else(|| ToolError::NotFound(format!("Tool not found: {}", call.name)))?;
        let args = serde_json::to_value(&call.arguments).unwrap_or(serde_json::Value::Object(Default::default()));
        tool.execute(args, context).await
    }
}
