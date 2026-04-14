// Re-export gateway types from hermes-core
pub use hermes_core::gateway::{GatewayError, InboundMessage};

/// Agent response wrapper for internal use.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub session_id: String,
}
