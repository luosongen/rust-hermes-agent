use crate::Usage;

/// Conversation request
#[derive(Debug, Clone)]
pub struct ConversationRequest {
    pub content: String,
    pub session_id: Option<String>,
    pub system_prompt: Option<String>,
}

/// Conversation response
#[derive(Debug, Clone)]
pub struct ConversationResponse {
    pub content: String,
    pub session_id: Option<String>,
    pub usage: Option<Usage>,
}
