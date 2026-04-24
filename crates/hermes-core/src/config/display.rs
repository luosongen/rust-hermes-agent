use serde::{Deserialize, Serialize};

/// Display configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_compact")]
    pub compact: bool,
    #[serde(default = "default_tool_progress")]
    pub tool_progress: bool,
    #[serde(default = "default_skin")]
    pub skin: String,
}

fn default_compact() -> bool {
    false
}
fn default_tool_progress() -> bool {
    true
}
fn default_skin() -> String {
    "default".to_string()
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            compact: false,
            tool_progress: true,
            skin: "default".to_string(),
        }
    }
}
