//! browser_tools — 浏览器自动化工具
//!
//! 通过 agent-browser CLI 调用本地 headless Chromium。

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use parking_lot::RwLock;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Session 超时时间（秒）
const INACTIVITY_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Deserialize)]
struct BrowserResponse {
    success: bool,
    #[serde(default)]
    data: serde_json::Value,
    #[serde(default)]
    error: Option<String>,
}

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

    /// Start the background cleanup task. Call once after core is created,
    /// inside a Tokio runtime. Tests skip this since they have no runtime.
    pub fn start_cleanup(&self) {
        let store = Arc::clone(&self.store);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let stale = {
                    let store = store.read();
                    store.get_stale_sessions()
                };
                if !stale.is_empty() {
                    let sessions_to_close: Vec<(String, String)> = {
                        let store = store.read();
                        stale
                            .into_iter()
                            .filter_map(|task_id| {
                                store.task_session_map.get(&task_id).cloned().map(|name| (task_id, name))
                            })
                            .collect()
                    };
                    for (task_id, session_name) in sessions_to_close {
                        let _ = tokio::process::Command::new("agent-browser")
                            .arg("--session")
                            .arg(&session_name)
                            .arg("--json")
                            .arg("close")
                            .output()
                            .await;
                        let mut store = store.write();
                        store.remove_session(&task_id);
                    }
                }
            }
        });
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

impl BrowserToolCore {
    /// Execute an agent-browser command.
    pub async fn run_command(
        &self,
        task_id: &str,
        subcmd: &str,
        args: Vec<&str>,
        timeout_secs: u64,
    ) -> Result<serde_json::Value, ToolError> {
        let (session_name, socket_dir) = {
            let mut store = self.store.write();
            // Special case: "open" creates session if not exists
            let session = if subcmd == "open" {
                if let Some(existing) = store.get_session(task_id).cloned() {
                    existing
                } else {
                    store.create_session(task_id)
                }
            } else {
                store.get_session(task_id)
                    .ok_or_else(|| ToolError::InvalidArgs("No active session. Call browser_navigate first.".into()))?
                    .clone()
            };
            store.touch(task_id);
            (session.session_name.clone(), session.socket_dir.clone())
        };

        let mut cmd = tokio::process::Command::new("agent-browser");
        cmd.arg("--session").arg(&session_name);
        cmd.arg("--json").arg(subcmd);
        cmd.args(&args);
        cmd.env("AGENT_BROWSER_SOCKET_DIR", socket_dir.to_string_lossy().as_ref());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            cmd.output(),
        )
        .await
        .map_err(|_| ToolError::Execution("Command timed out".into()))?
        .map_err(|e| ToolError::Execution(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!("agent-browser failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let resp: BrowserResponse = serde_json::from_str(&stdout)
            .map_err(|e| ToolError::Execution(format!("Invalid JSON: {}", e)))?;

        if !resp.success {
            return Err(ToolError::Execution(
                resp.error.unwrap_or_else(|| "Unknown error".into()),
            ));
        }

        Ok(resp.data)
    }
}

// === browser_navigate ===

pub struct BrowserNavigateTool {
    pub core: BrowserToolCore,
}

impl BrowserNavigateTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserNavigateTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserNavigateTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserNavigateTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateParams {
    pub url: String,
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }
    fn description(&self) -> &str {
        "Navigate to a URL in the browser."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" }
            },
            "required": ["url"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: NavigateParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let data = self
            .core
            .run_command(&context.session_id, "open", vec![&params.url], 60)
            .await?;

        Ok(json!({
            "success": true,
            "url": data.get("url").and_then(|v| v.as_str()).unwrap_or(&params.url),
            "title": data.get("title").and_then(|v| v.as_str()).unwrap_or(""),
        })
        .to_string())
    }
}

// === browser_snapshot ===

pub struct BrowserSnapshotTool {
    pub core: BrowserToolCore,
}

impl BrowserSnapshotTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserSnapshotTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserSnapshotTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserSnapshotTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotParams {
    #[serde(default)]
    pub full: bool,
}

#[async_trait]
impl Tool for BrowserSnapshotTool {
    fn name(&self) -> &str {
        "browser_snapshot"
    }
    fn description(&self) -> &str {
        "Get text-based accessibility tree snapshot."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "full": { "type": "boolean", "default": false }
            }
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: SnapshotParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let args_vec: Vec<&str> = if params.full { vec![] } else { vec!["-c"] };

        let data = self
            .core
            .run_command(&context.session_id, "snapshot", args_vec, 30)
            .await?;

        Ok(json!({
            "success": true,
            "snapshot": data.get("snapshot").and_then(|v| v.as_str()).unwrap_or(""),
            "element_count": data.get("refs").and_then(|v| v.as_array()).map(|a| a.len() as u64).unwrap_or(0)
        })
        .to_string())
    }
}

// === browser_click ===

pub struct BrowserClickTool {
    pub core: BrowserToolCore,
}

impl BrowserClickTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserClickTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserClickTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserClickTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickParams {
    pub r#ref: String,
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn name(&self) -> &str {
        "browser_click"
    }
    fn description(&self) -> &str {
        "Click element by ref ID."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "ref": { "type": "string" }
            },
            "required": ["ref"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: ClickParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
        let mut ref_str = params.r#ref;
        if !ref_str.starts_with('@') {
            ref_str = format!("@{}", ref_str);
        }
        self.core
            .run_command(&context.session_id, "click", vec![&ref_str], 30)
            .await?;
        Ok(json!({ "success": true, "clicked": ref_str }).to_string())
    }
}

// === browser_type ===

pub struct BrowserTypeTool {
    pub core: BrowserToolCore,
}

impl BrowserTypeTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserTypeTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserTypeTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserTypeTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeParams {
    pub r#ref: String,
    pub text: String,
}

#[async_trait]
impl Tool for BrowserTypeTool {
    fn name(&self) -> &str {
        "browser_type"
    }
    fn description(&self) -> &str {
        "Type text into input field by ref ID."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "ref": { "type": "string" },
                "text": { "type": "string" }
            },
            "required": ["ref", "text"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: TypeParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
        let mut ref_str = params.r#ref;
        if !ref_str.starts_with('@') {
            ref_str = format!("@{}", ref_str);
        }
        self.core
            .run_command(&context.session_id, "fill", vec![&ref_str, &params.text], 30)
            .await?;
        Ok(json!({ "success": true, "typed": params.text, "element": ref_str }).to_string())
    }
}

// === browser_scroll ===

pub struct BrowserScrollTool {
    pub core: BrowserToolCore,
}

impl BrowserScrollTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserScrollTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserScrollTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserScrollTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrollParams {
    pub direction: String,
}

#[async_trait]
impl Tool for BrowserScrollTool {
    fn name(&self) -> &str {
        "browser_scroll"
    }
    fn description(&self) -> &str {
        "Scroll page up or down."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "direction": { "type": "string", "enum": ["up", "down"] }
            },
            "required": ["direction"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: ScrollParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
        let dir = &params.direction;
        // agent-browser scroll only supports "up" or "down"
        // Note: schema enum restricts to ["up", "down"], runtime check is redundant but defensive
        if dir != "up" && dir != "down" {
            return Err(ToolError::InvalidArgs("direction must be 'up' or 'down'".into()));
        }
        self.core
            .run_command(&context.session_id, "scroll", vec![dir, "500"], 30)
            .await?;
        Ok(json!({ "success": true, "scrolled": dir }).to_string())
    }
}

// === browser_back ===

pub struct BrowserBackTool {
    pub core: BrowserToolCore,
}

impl BrowserBackTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserBackTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserBackTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserBackTool").finish()
    }
}

#[async_trait]
impl Tool for BrowserBackTool {
    fn name(&self) -> &str {
        "browser_back"
    }
    fn description(&self) -> &str {
        "Navigate back in browser history."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }
    async fn execute(&self, _args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        self.core
            .run_command(&context.session_id, "back", vec![], 30)
            .await?;
        Ok(json!({ "success": true }).to_string())
    }
}

// === browser_press ===

pub struct BrowserPressTool {
    pub core: BrowserToolCore,
}

impl BrowserPressTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserPressTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserPressTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserPressTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PressParams {
    pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionParams {
    pub question: String,
    #[serde(default)]
    pub annotate: bool,
}

#[async_trait]
impl Tool for BrowserPressTool {
    fn name(&self) -> &str {
        "browser_press"
    }
    fn description(&self) -> &str {
        "Press keyboard key."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "key": { "type": "string" }
            },
            "required": ["key"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: PressParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
        self.core
            .run_command(&context.session_id, "press", vec![&params.key], 30)
            .await?;
        Ok(json!({ "success": true, "pressed": params.key }).to_string())
    }
}

// === browser_vision ===

pub struct BrowserVisionTool {
    pub core: BrowserToolCore,
}

impl BrowserVisionTool {
    pub fn new(core: BrowserToolCore) -> Self {
        Self { core }
    }
}

impl Clone for BrowserVisionTool {
    fn clone(&self) -> Self {
        Self { core: self.core.clone() }
    }
}

impl std::fmt::Debug for BrowserVisionTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserVisionTool").finish()
    }
}

#[async_trait]
impl Tool for BrowserVisionTool {
    fn name(&self) -> &str {
        "browser_vision"
    }
    fn description(&self) -> &str {
        "Take screenshot and analyze with vision AI."
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "question": { "type": "string" },
                "annotate": { "type": "boolean", "default": false }
            },
            "required": ["question"]
        })
    }
    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: VisionParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let screenshot_dir = std::env::temp_dir().join("hermes-screenshots");
        std::fs::create_dir_all(&screenshot_dir).ok();
        let screenshot_path = screenshot_dir.join(format!("browser_{}.png", Uuid::new_v4()));

        let mut screenshot_args = vec![];
        if params.annotate {
            screenshot_args.push("--annotate");
        }
        screenshot_args.push("--full");
        screenshot_args.push(screenshot_path.to_str().unwrap());

        self.core
            .run_command(&context.session_id, "screenshot", screenshot_args, 60)
            .await?;

        if !screenshot_path.exists() {
            return Err(ToolError::Execution("Screenshot file not created".into()));
        }

        Ok(json!({
            "success": true,
            "screenshot_path": screenshot_path.to_str().unwrap_or(""),
            "analysis": "Screenshot captured. Vision analysis not yet integrated."
        }).to_string())
    }
}
