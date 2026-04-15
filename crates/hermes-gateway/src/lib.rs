//! ## hermes-gateway
//!
//! HTTP 网关模块，负责接收各平台（Telegram、WeCom）的 Webhook 请求，
//! 并将消息转发给 `Agent` 进行处理。
//!
//! ### 主要职责
//! - 提供 HTTP 接口 `/health`、`/webhook/telegram`、`/webhook/wecom`
//! - 管理多个平台适配器（`PlatformAdapter`），根据平台标识路由请求
//! - 验证 Webhook 请求的合法性
//! - 将入站消息转换为 `InboundMessage`，交由 Agent 处理后回传响应
//!
//! ### 请求流程
//! ```text
//! Webhook 请求 → 适配器验证(verify_webhook) → 解析(parse_inbound)
//!     → Agent.run_conversation() → 发送响应(send_response)
//! ```

pub mod error;
pub mod types;

pub use error::GatewayError;
pub use types::{AgentResponse, InboundMessage};

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::Response,
    routing::post,
    Router,
};
use hermes_core::{
    gateway::PlatformAdapter,
    Agent, ConversationRequest,
};
use parking_lot::RwLock;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

/// The gateway application — holds adapters and agent reference.
pub struct Gateway {
    adapters: RwLock<Vec<Arc<dyn PlatformAdapter>>>,
    agent: Arc<Agent>,
}

impl Gateway {
    pub fn new(agent: Arc<Agent>) -> Self {
        Self {
            adapters: RwLock::new(Vec::new()),
            agent,
        }
    }

    pub fn register_adapter(&self, adapter: Arc<dyn PlatformAdapter>) {
        self.adapters.write().push(adapter);
    }

    /// Build the axum Router for the gateway.
    pub fn router(self: Arc<Self>) -> Router {
        let middleware = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .into_inner();

        Router::new()
            .route("/health", axum::routing::get(health_handler))
            .route("/webhook/telegram", post(webhook_telegram))
            .route("/webhook/wecom", post(webhook_wecom))
            .with_state(Arc::new(GatewayState { gateway: self }))
            .layer(middleware)
    }
}

struct GatewayState {
    gateway: Arc<Gateway>,
}

async fn health_handler() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"status":"ok"}"#))
        .unwrap()
}

async fn webhook_telegram(
    State(state): axum::extract::State<Arc<GatewayState>>,
    request: Request<Body>,
) -> Response {
    handle_webhook(&state.gateway, "telegram", request).await
}

async fn webhook_wecom(
    State(state): axum::extract::State<Arc<GatewayState>>,
    request: Request<Body>,
) -> Response {
    handle_webhook(&state.gateway, "wecom", request).await
}

async fn handle_webhook(
    gateway: &Gateway,
    platform: &str,
    request: Request<Body>,
) -> Response {
    let adapter = {
        let adapters = gateway.adapters.read();
        adapters
            .iter()
            .find(|a| a.platform_id() == platform)
            .cloned()
    };

    let adapter = match adapter {
        Some(a) => a,
        None => {
            tracing::error!("No adapter registered for platform: {}", platform);
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(r#"{"error":"no adapter"}"#))
                .unwrap();
        }
    };

    if !adapter.verify_webhook(&request) {
        tracing::warn!("Webhook verification failed for {}", platform);
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::from(r#"{"error":"unauthorized"}"#))
            .unwrap();
    }

    let inbound = match adapter.parse_inbound(request).await {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!("Failed to parse inbound from {}: {}", platform, e);
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!(r#"{{"error":"{}"}}"#, e)))
                .unwrap();
        }
    };

    tracing::info!(
        "Received {} message from {} (session={})",
        inbound.platform,
        inbound.sender_id,
        inbound.session_id
    );

    let response = gateway
        .agent
        .run_conversation(ConversationRequest {
            content: inbound.content.clone(),
            session_id: Some(inbound.session_id.clone()),
            system_prompt: None,
        })
        .await;

    match response {
        Ok(resp) => {
            if let Err(e) = adapter.send_response(resp, &inbound).await {
                tracing::error!("Failed to send response: {}", e);
            }
            Response::builder()
                .status(StatusCode::OK)
                .body(Body::from(r#"{"ok":true}"#))
                .unwrap()
        }
        Err(e) => {
            tracing::error!("Agent error: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!(r#"{{"error":"{}"}}"#, e)))
                .unwrap()
        }
    }
}
