//! Hermes Agent 配置系统
//!
//! 配置文件位置: `~/.config/hermes-agent/config.toml` (符合 XDG 标准)
//!
//! 配置优先级（从高到低）:
//! 1. CLI 参数
//! 2. 环境变量 (HERMES_*)
//! 3. 配置文件
//! 4. 默认值

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use parking_lot::Mutex;
use std::sync::OnceLock;

/// 配置缓存（使用 OnceLock 实现懒加载）
static CONFIG_CACHE: OnceLock<Mutex<Config>> = OnceLock::new();

/// 获取默认配置目录（符合 XDG 标准）
/// 优先使用 XDG_CONFIG_HOME，如果没有则回退到 ~/.config
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("hermes-agent")
}

/// 获取默认配置文件路径
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// 消息平台的配置结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformConfig {
    pub bot_token: Option<String>,      // Telegram bot token
    pub verify_token: Option<String>,   // Telegram webhook 验证 token
    pub corp_id: Option<String>,       // WeCom 企业 ID
    pub agent_id: Option<String>,      // WeCom 应用 agent ID
    pub token: Option<String>,         // WeCom token
    pub aes_key: Option<String>,       // WeCom AES 密钥
}

/// 网关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub port: u16,                          // 网关端口
    pub host: String,                       // 网关主机地址
    pub platforms: HashMap<String, PlatformConfig>,  // 已配置的平台
}

/// 默认配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    pub model: String,        // 默认模型（如 "openai/gpt-4o"）
    pub tools_enabled: bool,  // 是否启用工具
}

/// 主要配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub credentials: HashMap<String, String>,
    #[serde(default)]
    pub gateway: GatewayConfig,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            model: "openai/gpt-4o".to_string(),
            tools_enabled: true,
        }
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "0.0.0.0".to_string(),
            platforms: HashMap::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: DefaultsConfig::default(),
            credentials: HashMap::new(),
            gateway: GatewayConfig::default(),
        }
    }
}

impl Config {
    /// 从所有来源加载配置
    ///
    /// 优先级顺序（从高到低）:
    /// 1. CLI 参数（由调用方单独处理）
    /// 2. 环境变量 (HERMES_*)
    /// 3. 配置文件 (~/.config/hermes-agent/config.toml)
    /// 4. 默认值
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Config::default();

        // 1. 从配置文件加载（如果存在）
        let path = config_file();
        if path.exists() {
            let content = fs::read_to_string(&path).map_err(ConfigError::Io)?;
            // 优先尝试 TOML 格式，然后尝试 YAML
            if let Ok(file_config) = toml::from_str::<Config>(&content) {
                config.merge(file_config);
            } else if let Ok(file_config) = serde_yaml::from_str::<Config>(&content) {
                config.merge(file_config);
            }
        }

        // 2. 用环境变量覆盖
        config.load_from_env();

        Ok(config)
    }

    /// 将另一个配置合并到此配置中（只覆盖已设置的值）
    fn merge(&mut self, other: Config) {
        // 只覆盖非默认值的字段
        if other.defaults.model != DefaultsConfig::default().model {
            self.defaults.model = other.defaults.model;
        }
        if other.defaults.tools_enabled != DefaultsConfig::default().tools_enabled {
            self.defaults.tools_enabled = other.defaults.tools_enabled;
        }
        if !other.credentials.is_empty() {
            self.credentials.extend(other.credentials);
        }
        if other.gateway.port != GatewayConfig::default().port {
            self.gateway.port = other.gateway.port;
        }
        if other.gateway.host != GatewayConfig::default().host {
            self.gateway.host = other.gateway.host;
        }
        for (name, platform) in other.gateway.platforms {
            self.gateway.platforms.insert(name, platform);
        }
    }

    /// 从 HERMES_* 环境变量加载配置
    fn load_from_env(&mut self) {
        if let Ok(val) = std::env::var("HERMES_DEFAULT_MODEL") {
            self.defaults.model = val;
        }
        if let Ok(val) = std::env::var("HERMES_TOOLS_ENABLED") {
            self.defaults.tools_enabled = val != "false";
        }
        if let Ok(val) = std::env::var("HERMES_GATEWAY_PORT") {
            if let Ok(port) = val.parse() {
                self.gateway.port = port;
            }
        }
        if let Ok(val) = std::env::var("HERMES_GATEWAY_HOST") {
            self.gateway.host = val;
        }
        if let Ok(val) = std::env::var("HERMES_OPENAI_API_KEY") {
            self.credentials.insert("openai".to_string(), val);
        }
        if let Ok(val) = std::env::var("HERMES_ANTHROPIC_API_KEY") {
            self.credentials.insert("anthropic".to_string(), val);
        }
        if let Ok(val) = std::env::var("HERMES_TELEGRAM_BOT_TOKEN") {
            self.gateway.platforms.entry("telegram".to_string()).or_default();
            if let Some(p) = self.gateway.platforms.get_mut("telegram") {
                p.bot_token = Some(val);
            }
        }
        if let Ok(val) = std::env::var("HERMES_TELEGRAM_VERIFY_TOKEN") {
            self.gateway.platforms.entry("telegram".to_string()).or_default();
            if let Some(p) = self.gateway.platforms.get_mut("telegram") {
                p.verify_token = Some(val);
            }
        }
    }

    /// 通过键获取配置值（支持点号分隔的嵌套键）
    ///
    /// 示例:
    /// - `defaults.model`
    /// - `gateway.port`
    /// - `credentials.openai`
    pub fn get(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["defaults", "model"] => Some(self.defaults.model.clone()),
            ["defaults", "tools_enabled"] => Some(self.defaults.tools_enabled.to_string()),
            ["gateway", "port"] => Some(self.gateway.port.to_string()),
            ["gateway", "host"] => Some(self.gateway.host.clone()),
            ["credentials", name] => self.credentials.get(name as &str).cloned(),
            ["gateway", "platforms", name, field] => {
                self.gateway.platforms.get(name as &str).and_then(|p| match *field {
                    "bot_token" => p.bot_token.clone(),
                    "verify_token" => p.verify_token.clone(),
                    "corp_id" => p.corp_id.clone(),
                    "agent_id" => p.agent_id.clone(),
                    "token" => p.token.clone(),
                    "aes_key" => p.aes_key.clone(),
                    _ => None,
                })
            }
            _ => None,
        }
    }

    /// 通过键设置配置值（支持点号分隔的嵌套键）
    pub fn set(&mut self, key: &str, value: String) -> bool {
        let parts: Vec<&str> = key.split('.').collect();
        match parts.as_slice() {
            ["defaults", "model"] => {
                self.defaults.model = value;
                true
            }
            ["defaults", "tools_enabled"] => {
                self.defaults.tools_enabled = value.parse().unwrap_or(true);
                true
            }
            ["gateway", "port"] => {
                self.gateway.port = value.parse().unwrap_or(8080);
                true
            }
            ["gateway", "host"] => {
                self.gateway.host = value;
                true
            }
            ["credentials", name] => {
                self.credentials.insert(name.to_string(), value);
                true
            }
            _ => false,
        }
    }

    /// 保存配置到配置文件
    ///
    /// 如果配置目录不存在会创建它。
    /// 设置文件权限为 0o600（仅用户可读写）以保证安全。
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = config_file();

        // 如果配置目录不存在则创建
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(ConfigError::Io)?;
        }

        // 序列化为 TOML 格式
        let toml_str = toml::to_string_pretty(self).map_err(|e| ConfigError::Serialize(e.to_string()))?;

        // 写入并设置安全权限
        fs::write(&path, &toml_str).map_err(ConfigError::Io)?;

        // 设置权限为 0o600（仅用户读写）
        fs::set_permissions(&path, PermissionsExt::from_mode(0o600))
            .map_err(ConfigError::Io)?;

        Ok(())
    }

    /// 获取缓存的配置实例（首次调用时加载）
    pub fn get_cached() -> &'static Mutex<Config> {
        CONFIG_CACHE.get_or_init(|| {
            Mutex::new(Self::load().expect("failed to load config"))
        })
    }

    /// 清除缓存的配置（用于测试）
    #[allow(dead_code)]
    pub fn clear_cache() {
        // OnceLock 不支持 take，我们只是在测试时让整个 static 自然结束
    }

    /// 从 $EDITOR 环境变量获取编辑器，回退到 $VISUAL，然后是合理的默认值
    pub fn editor() -> String {
        std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string())
    }

    /// 格式化配置用于显示，会对敏感值进行脱敏处理
    pub fn display(&self) -> String {
        let mut lines = vec![];

        lines.push("[defaults]".to_string());
        lines.push(format!("  model = \"{}\"", self.defaults.model));
        lines.push(format!("  tools_enabled = {}", self.defaults.tools_enabled));

        lines.push("\n[gateway]".to_string());
        lines.push(format!("  port = {}", self.gateway.port));
        lines.push(format!("  host = \"{}\"", self.gateway.host));

        if !self.gateway.platforms.is_empty() {
            lines.push("  [gateway.platforms]".to_string());
            for (name, platform) in &self.gateway.platforms {
                lines.push(format!("    [[gateway.platforms.{}]]", name));
                if let Some(ref token) = platform.bot_token {
                    lines.push(format!("      bot_token = \"{}\"", redact_secret(token)));
                }
                if let Some(ref token) = platform.verify_token {
                    lines.push(format!("      verify_token = \"{}\"", redact_secret(token)));
                }
                if let Some(ref id) = platform.corp_id {
                    lines.push(format!("      corp_id = \"{}\"", redact_secret(id)));
                }
                if let Some(ref id) = platform.agent_id {
                    lines.push(format!("      agent_id = \"{}\"", redact_secret(id)));
                }
                if let Some(ref token) = platform.token {
                    lines.push(format!("      token = \"{}\"", redact_secret(token)));
                }
                if let Some(ref key) = platform.aes_key {
                    lines.push(format!("      aes_key = \"{}\"", redact_secret(key)));
                }
            }
        }

        lines.push("\n[credentials]".to_string());
        for (name, value) in &self.credentials {
            lines.push(format!("  {} = \"{}\"", name, redact_secret(value)));
        }

        lines.join("\n")
    }
}

/// 对敏感值进行脱敏处理（只显示前5个字符）
fn redact_secret(value: &str) -> String {
    if value.len() <= 9 {
        "*".repeat(value.len())
    } else {
        format!("{}...****", &value[..5])
    }
}

/// 配置错误类型
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("序列化配置失败: {0}")]
    Serialize(String),
    #[error("I/O 操作失败: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_get_set() {
        let mut config = Config::default();
        assert_eq!(config.get("defaults.model"), Some("openai/gpt-4o".to_string()));

        config.set("defaults.model", "anthropic/claude-3".to_string());
        assert_eq!(config.get("defaults.model"), Some("anthropic/claude-3".to_string()));
    }

    #[test]
    fn test_credentials_redaction() {
        let redacted = redact_secret("sk-1234567890abcdef");
        assert_eq!(redacted, "sk-12...****");
    }

    #[test]
    fn test_config_display_redacts_credentials() {
        let mut config = Config::default();
        config.credentials.insert("openai".to_string(), "sk-abcdef123456".to_string());

        let display = config.display();
        assert!(display.contains("sk-ab...****"));
        assert!(!display.contains("sk-abcdef123456"));
    }
}
