//! approval_tools — 危险命令审批工具

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_tool_registry::Tool;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// 预编译的危险命令模式：(编译后的 Regex, 描述)
static COMPILED_PATTERNS: Lazy<Vec<(Regex, &'static str)>> = Lazy::new(|| {
    vec![
        // 删除类
        (Regex::new(r"\brm\s+(-[^\s]*\s*)*/").expect("valid regex"), "delete in root path"),
        (Regex::new(r"\brm\s+-[^\s]*r").expect("valid regex"), "recursive delete"),
        (Regex::new(r"\brm\s+--recursive\b").expect("valid regex"), "recursive delete (long flag)"),
        (Regex::new(r"\brm\s+-[^\s]*f").expect("valid regex"), "force delete"),
        (Regex::new(r"\brmdir\b").expect("valid regex"), "remove directories"),
        // 权限类
        (Regex::new(r"\bchmod\s+(-[^\s]*\s*)*(777|666|o\+[rwx]*w|a\+[rwx]*w)").expect("valid regex"), "world-writable permissions"),
        (Regex::new(r"\bchmod\s+--recursive\b.*(777|666)").expect("valid regex"), "recursive chmod 777/666"),
        (Regex::new(r"\bchown\s+(-[^\s]*\s*)*[^\s]+\s+[^\s]+:[^\s]+").expect("valid regex"), "change ownership"),
        // 管道注入类
        (Regex::new(r"\bcurl\s+.*\|\s*bash").expect("valid regex"), "pipe to bash (curl | bash)"),
        (Regex::new(r"\bwget\s+.*\|\s*bash").expect("valid regex"), "pipe to bash (wget | bash)"),
        (Regex::new(r"\bfetch\s+.*\|\s*bash").expect("valid regex"), "pipe to bash (fetch | bash)"),
        // 提权类
        (Regex::new(r"\bsudo\s+su\b").expect("valid regex"), "sudo su"),
        (Regex::new(r"\bsu\s+-\s*root").expect("valid regex"), "switch to root"),
        // 系统文件类
        (Regex::new(r"\bnano\s+/etc/sudoers").expect("valid regex"), "edit sudoers file"),
        (Regex::new(r"\bvim?\s+/etc/sudoers").expect("valid regex"), "edit sudoers file (vim)"),
        (Regex::new(r"\btee\s+.*/etc/").expect("valid regex"), "write to system directory"),
        (Regex::new(r"\bcat\s+.*>\s*/etc/").expect("valid regex"), "redirect to system file"),
        // 网络类
        (Regex::new(r"\biptables\s+(-[^\s]*\s*)*F").expect("valid regex"), "flush iptables rules"),
        (Regex::new(r"\bufw\s+disable").expect("valid regex"), "disable firewall"),
        // 进程类
        (Regex::new(r"\bpkill\s+(-[^\s]*\s*)*-9").expect("valid regex"), "force kill process"),
        (Regex::new(r"\bkill\s+(-[^\s]*\s*)*-9").expect("valid regex"), "force kill process"),
        (Regex::new(r"\bkillall\b").expect("valid regex"), "kill all processes"),
        // 格式化类
        (Regex::new(r"\bmkfs\b").expect("valid regex"), "format filesystem"),
        (Regex::new(r"\bmke2fs\b").expect("valid regex"), "format ext filesystem"),
        (Regex::new(r"\bdd\s+.*of=/dev/").expect("valid regex"), "direct disk write"),
        // 服务类
        (Regex::new(r"\bsystemctl\s+(stop|disable).*").expect("valid regex"), "stop/disable service"),
        (Regex::new(r"\bservice\s+.*stop").expect("valid regex"), "stop service"),
    ]
});

/// 检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub needs_approval: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_matched: Option<String>,
}

/// 待审批命令
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCommand {
    pub command: String,
    pub session_key: String,
    pub timestamp: f64,
    pub status: String,
}

/// 审批状态（内存）
#[derive(Debug, Default)]
pub struct ApprovalState {
    pub pending: HashMap<String, Vec<PendingCommand>>,
    pub approved: HashMap<String, HashMap<String, f64>>,
    pub denied: HashMap<String, HashSet<String>>,
}

/// 审批存储
#[derive(Debug)]
pub struct ApprovalStore {
    state: Arc<RwLock<ApprovalState>>,
}

impl ApprovalStore {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(ApprovalState::default())),
        }
    }

    pub fn check(&self, command: &str) -> CheckResult {
        for (re, description) in COMPILED_PATTERNS.iter() {
            if re.is_match(command) {
                return CheckResult {
                    needs_approval: true,
                    reason: Some((*description).to_string()),
                    pattern_matched: Some(re.as_str().to_string()),
                };
            }
        }
        CheckResult {
            needs_approval: false,
            reason: None,
            pattern_matched: None,
        }
    }

    pub fn approve(&mut self, command: &str, session_key: &str) {
        let cmd_hash = hash_command(command);
        let timestamp = now();
        let mut state = self.state.write();
        state
            .approved
            .entry(session_key.to_string())
            .or_default()
            .insert(cmd_hash, timestamp);
        if let Some(pending) = state.pending.get_mut(session_key) {
            pending.retain(|p| p.command != command);
        }
        if let Some(denied) = state.denied.get_mut(session_key) {
            denied.remove(&hash_command(command));
        }
    }

    pub fn deny(&mut self, command: &str, session_key: &str) {
        let cmd_hash = hash_command(command);
        let mut state = self.state.write();
        state
            .denied
            .entry(session_key.to_string())
            .or_default()
            .insert(cmd_hash);
    }

    pub fn is_whitelisted(&self, command: &str, session_key: &str) -> bool {
        let cmd_hash = hash_command(command);
        let state = self.state.read();
        state
            .approved
            .get(session_key)
            .map(|h| h.contains_key(&cmd_hash))
            .unwrap_or(false)
    }

    pub fn is_denied(&self, command: &str, session_key: &str) -> bool {
        let cmd_hash = hash_command(command);
        let state = self.state.read();
        state
            .denied
            .get(session_key)
            .map(|s| s.contains(&cmd_hash))
            .unwrap_or(false)
    }

    pub fn add_pending(&mut self, command: String, session_key: &str) {
        let pending_item = PendingCommand {
            command,
            session_key: session_key.to_string(),
            timestamp: now(),
            status: "pending".to_string(),
        };
        let mut state = self.state.write();
        state.pending.entry(session_key.to_string()).or_default().push(pending_item);
    }

    pub fn list_pending(&self, session_key: &str) -> Vec<PendingCommand> {
        let state = self.state.read();
        state.pending.get(session_key).cloned().unwrap_or_default()
    }

    pub fn load_whitelist(&mut self, whitelist: HashMap<String, f64>) {
        let mut state = self.state.write();
        state.approved.insert("default".to_string(), whitelist);
    }

    pub fn get_whitelist(&self) -> HashMap<String, f64> {
        let state = self.state.read();
        state.approved.get("default").cloned().unwrap_or_default()
    }
}

impl Default for ApprovalStore {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_command(cmd: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    cmd.hash(&mut h);
    format!("{:x}", h.finish())
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// ApprovalTool
pub struct ApprovalTool {
    store: Arc<RwLock<ApprovalStore>>,
    config_path: std::path::PathBuf,
}

impl ApprovalTool {
    pub fn new(config_dir: std::path::PathBuf) -> Self {
        let mut store = ApprovalStore::new();
        let whitelist_path = config_dir.join("approval_whitelist.toml");
        if let Ok(content) = std::fs::read_to_string(&whitelist_path) {
            if let Ok(whitelist) = toml::from_str::<HashMap<String, f64>>(&content) {
                store.load_whitelist(whitelist);
            }
        }
        Self {
            store: Arc::new(RwLock::new(store)),
            config_path: whitelist_path,
        }
    }

    fn save_whitelist(&self) {
        let whitelist = {
            let store = self.store.read();
            store.get_whitelist()
        };
        if let Ok(content) = toml::to_string_pretty(&whitelist) {
            let _ = std::fs::write(&self.config_path, content);
        }
    }
}

impl Clone for ApprovalTool {
    fn clone(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
            config_path: self.config_path.clone(),
        }
    }
}

impl std::fmt::Debug for ApprovalTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalTool").finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalParams {
    pub action: String,
    #[serde(default)]
    pub command: Option<String>,
}

#[async_trait]
impl Tool for ApprovalTool {
    fn name(&self) -> &str {
        "approval"
    }

    fn description(&self) -> &str {
        "Check, approve, or deny dangerous commands before execution."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["check", "approve", "deny", "list"],
                    "description": "check: verify if command needs approval. approve: allow command. deny: reject command. list: show pending commands."
                },
                "command": {
                    "type": "string",
                    "description": "The command to check/approve/deny."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value, context: ToolContext) -> Result<String, ToolError> {
        let params: ApprovalParams =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let session_key = context.session_id.as_str();

        match params.action.as_str() {
            "check" => {
                let command = params.command.ok_or_else(|| ToolError::InvalidArgs("command required for check".into()))?;
                let result = self.store.read().check(&command);
                if result.needs_approval {
                    let mut store = self.store.write();
                    store.add_pending(command.clone(), session_key);
                }
                Ok(json!({
                    "needs_approval": result.needs_approval,
                    "reason": result.reason,
                    "pattern_matched": result.pattern_matched
                }).to_string())
            }
            "approve" => {
                let command = params.command.ok_or_else(|| ToolError::InvalidArgs("command required for approve".into()))?;
                {
                    let mut store = self.store.write();
                    store.approve(&command, session_key);
                }
                self.save_whitelist();
                Ok(json!({"status": "approved", "command": command, "whitelisted": true}).to_string())
            }
            "deny" => {
                let command = params.command.ok_or_else(|| ToolError::InvalidArgs("command required for deny".into()))?;
                {
                    let mut store = self.store.write();
                    store.deny(&command, session_key);
                }
                Ok(json!({"status": "denied", "command": command}).to_string())
            }
            "list" => {
                let pending = self.store.read().list_pending(session_key);
                Ok(json!({"pending": pending}).to_string())
            }
            _ => Err(ToolError::InvalidArgs(format!("unknown action: {}", params.action))),
        }
    }
}
