//! Messaging gateway types — shared between hermes-gateway and platform adapters.
//!
//! This module lives in hermes-core to avoid cyclic dependencies:
//! hermes-gateway ↔ hermes-platform-telegram ↔ hermes-gateway

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::Request;
use chrono::{DateTime, Utc};
use crate::ConversationResponse;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Webhook verification failed: {0}")]
    VerificationFailed(String),

    #[error("Failed to parse inbound message: {0}")]
    ParseError(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Session error: {0}")]
    SessionError(String),

    #[error("Outbound error: {0}")]
    OutboundError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

/// Canonical inbound message after platform adapter parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub platform: String,
    pub sender_id: String,
    pub content: String,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub raw: serde_json::Value,
}

/// PlatformAdapter trait — implemented by each platform.
#[async_trait]
pub trait PlatformAdapter: Send + Sync {
    /// Human-readable platform name ("telegram" or "wecom").
    fn platform_id(&self) -> &str;

    /// Verify the webhook request is authentic.
    fn verify_webhook(&self, request: &Request<Body>) -> bool;

    /// Parse an inbound webhook request into a canonical InboundMessage.
    async fn parse_inbound(
        &self,
        request: Request<Body>,
    ) -> Result<InboundMessage, GatewayError>;

    /// Send an AgentResponse back to the platform.
    async fn send_response(
        &self,
        response: ConversationResponse,
        message: &InboundMessage,
    ) -> Result<(), GatewayError>;
}
