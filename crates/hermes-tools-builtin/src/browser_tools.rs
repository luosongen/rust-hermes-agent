//! browser_tools — 浏览器自动化工具
//!
//! 通过 agent-browser CLI 调用本地 headless Chromium。

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Session 超时时间（秒）
const INACTIVITY_TIMEOUT_SECS: u64 = 300;

/// 浏览器会话
#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub session_name: String,
    pub task_id: String,
    pub socket_dir: PathBuf,
    pub created_at: f64,
    pub last_activity: f64,
}

/// 会话存储
#[derive(Debug, Default)]
pub struct BrowserSessionStore {
    sessions: HashMap<String, BrowserSession>,
    task_session_map: HashMap<String, String>,
}

impl BrowserSessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建新会话
    pub fn create_session(&mut self, task_id: &str) -> BrowserSession {
        let session_name = format!("h_{}", &Uuid::new_v4().to_string()[..10]);
        let socket_dir = std::env::temp_dir().join(format!("agent-browser-{}", session_name));
        let now = now();
        let session = BrowserSession {
            session_name: session_name.clone(),
            task_id: task_id.to_string(),
            socket_dir,
            created_at: now,
            last_activity: now,
        };
        self.sessions.insert(session_name.clone(), session.clone());
        self.task_session_map.insert(task_id.to_string(), session_name);
        session
    }

    /// 获取会话（通过 task_id）
    pub fn get_session(&self, task_id: &str) -> Option<&BrowserSession> {
        let session_name = self.task_session_map.get(task_id)?;
        self.sessions.get(session_name)
    }

    /// 获取可变会话引用
    pub fn get_session_mut(&mut self, task_id: &str) -> Option<&mut BrowserSession> {
        let session_name = self.task_session_map.get(task_id)?;
        self.sessions.get_mut(session_name)
    }

    /// 更新最后活动时间
    pub fn touch(&mut self, task_id: &str) {
        if let Some(session) = self.get_session_mut(task_id) {
            session.last_activity = now();
        }
    }

    /// 删除会话
    pub fn remove_session(&mut self, task_id: &str) {
        if let Some(session_name) = self.task_session_map.remove(task_id) {
            self.sessions.remove(&session_name);
        }
    }

    /// 获取过期会话的 task_id 列表
    pub fn get_stale_sessions(&self) -> Vec<String> {
        let now = now();
        self.sessions
            .iter()
            .filter(|(_, s)| now - s.last_activity > INACTIVITY_TIMEOUT_SECS as f64)
            .map(|(_, s)| s.task_id.clone())
            .collect()
    }

    /// 清理过期会话
    pub fn cleanup_stale(&mut self) {
        let stale = self.get_stale_sessions();
        for task_id in stale {
            self.remove_session(&task_id);
        }
    }

    /// Force-set last_activity of a session to a given Unix timestamp (for testing stale detection).
    pub fn set_session_last_activity(&mut self, task_id: &str, time: f64) {
        let session_name = self.task_session_map.get(task_id);
        if let Some(name) = session_name {
            if let Some(session) = self.sessions.get_mut(name) {
                session.last_activity = time;
            }
        }
    }
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// BrowserToolCore — 共享浏览器核心逻辑（供所有 browser 工具使用）
pub struct BrowserToolCore {
    pub store: Arc<RwLock<BrowserSessionStore>>,
    config_dir: PathBuf,
}

impl BrowserToolCore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            store: Arc::new(RwLock::new(BrowserSessionStore::new())),
            config_dir,
        }
    }
}

impl Clone for BrowserToolCore {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            config_dir: self.config_dir.clone(),
        }
    }
}
