use async_trait::async_trait;
use crate::{ToolCall, ToolContext, ToolDefinition, ToolError};

/// Abstraction over the tool registry so hermes-core's Agent does not need
/// to depend on hermes-tool-registry (which already depends on hermes-core).
#[async_trait]
pub trait ToolDispatcher: Send + Sync {
    /// Return tool definitions to send to the LLM.
    fn get_definitions(&self) -> Vec<ToolDefinition>;

    /// Execute a tool call and return its string output.
    async fn dispatch(
        &self,
        call: &ToolCall,
        context: ToolContext,
    ) -> Result<String, ToolError>;
}
