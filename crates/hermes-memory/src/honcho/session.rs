//! Honcho session manager

use std::sync::Arc;
use super::client::HonchoClient;

pub struct HonchoSessionManager {
    client: Arc<HonchoClient>,
    user_peer_id: String,
}

impl HonchoSessionManager {
    pub fn new(client: Arc<HonchoClient>, user_peer_id: String) -> Self {
        Self { client, user_peer_id }
    }

    pub fn get_or_create_session(&self, session_id: &str) -> HonchoSession {
        HonchoSession {
            client: Arc::clone(&self.client),
            user_peer_id: self.user_peer_id.clone(),
            session_id: session_id.to_string(),
        }
    }

    pub async fn prefetch_dialectic(&self, query: &str, session_id: &str) -> String {
        let session = self.get_or_create_session(session_id);
        session.prefetch_dialectic(query).await.unwrap_or_default()
    }
}

pub struct HonchoSession {
    client: Arc<HonchoClient>,
    user_peer_id: String,
    session_id: String,
}

impl HonchoSession {
    pub async fn prefetch_dialectic(&self, query: &str) -> Result<String, String> {
        self.client.dialectic(query, &self.user_peer_id, 1).await
    }

    pub async fn get_context(&self, query: &str) -> Result<String, String> {
        self.client.search(query, &self.user_peer_id).await
    }

    pub async fn create_conclusion(&self, fact: &str) -> Result<(), String> {
        self.client.conclude(fact, &self.user_peer_id).await
    }
}