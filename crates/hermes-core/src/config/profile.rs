//! Profile 配置系统
//!
//! 支持多配置文件切换，适应不同使用场景。
//!
//! ## 配置文件位置
//! - 主配置: `~/.config/hermes-agent/config.toml`
//! - Profile 配置: `~/.config/hermes-agent/profiles/<name>.toml`
//!
//! ## 使用方式
//! - `/profile <name>` - 切换到指定配置
//! - `/profiles` - 列出所有配置
//!
//! ## Profile 结构
//! Profile 只包含需要覆盖的配置项，其他使用主配置默认值。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use super::Config;

/// Profile 配置结构
///
/// 每个 Profile 只包含需要覆盖的配置项。
/// 未设置的项将使用主配置的默认值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile 名称
    pub name: String,
    /// Profile 描述
    #[serde(default)]
    pub description: Option<String>,
    /// 覆盖的默认模型
    #[serde(default)]
    pub model: Option<String>,
    /// 覆盖的系统提示
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// 覆盖的工具启用状态
    #[serde(default)]
    pub tools_enabled: Option<bool>,
    /// 覆盖的 YOLO 模式
    #[serde(default)]
    pub yolo_mode: Option<bool>,
    /// 覆盖的 Fast 模式
    #[serde(default)]
    pub fast_mode: Option<bool>,
    /// 覆盖的温度参数
    #[serde(default)]
    pub temperature: Option<f32>,
    /// 覆盖的最大 Token 数
    #[serde(default)]
    pub max_tokens: Option<u32>,
    /// 关联的消息平台
    #[serde(default)]
    pub platform: Option<String>,
    /// 自定义配置项
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

impl Profile {
    /// 创建新的 Profile
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            model: None,
            system_prompt: None,
            tools_enabled: None,
            yolo_mode: None,
            fast_mode: None,
            temperature: None,
            max_tokens: None,
            platform: None,
            custom: HashMap::new(),
        }
    }

    /// 设置描述
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置模型
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置系统提示
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 设置 YOLO 模式
    pub fn with_yolo(mut self, yolo: bool) -> Self {
        self.yolo_mode = Some(yolo);
        self
    }

    /// 设置 Fast 模式
    pub fn with_fast(mut self, fast: bool) -> Self {
        self.fast_mode = Some(fast);
        self
    }

    /// 从文件加载 Profile
    pub fn from_file(path: &PathBuf) -> Result<Self, ProfileError> {
        let content = fs::read_to_string(path).map_err(ProfileError::Io)?;
        toml::from_str(&content).map_err(|e| ProfileError::Parse(e.to_string()))
    }

    /// 保存 Profile 到文件
    pub fn save(&self, path: &PathBuf) -> Result<(), ProfileError> {
        let parent = path.parent().ok_or_else(|| ProfileError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "无法获取父目录",
        )))?;

        fs::create_dir_all(parent).map_err(ProfileError::Io)?;

        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| ProfileError::Serialize(e.to_string()))?;

        fs::write(path, toml_str).map_err(ProfileError::Io)?;

        Ok(())
    }
}

/// Profile 管理器
///
/// 管理多个配置 Profile，支持加载、切换、合并配置。
pub struct ProfileManager {
    /// 已加载的 Profile 列表
    profiles: HashMap<String, Profile>,
    /// 当前激活的 Profile 名称
    active_profile: Option<String>,
    /// 主配置
    base_config: Arc<Config>,
}

impl ProfileManager {
    /// 创建新的 ProfileManager
    pub fn new(base_config: Arc<Config>) -> Self {
        Self {
            profiles: HashMap::new(),
            active_profile: None,
            base_config,
        }
    }

    /// 从配置目录加载所有 Profile
    ///
    /// Profile 文件位置: `~/.config/hermes-agent/profiles/*.toml`
    pub fn load_from_dir(&mut self) -> Result<(), ProfileError> {
        let profiles_dir = super::config_dir().join("profiles");

        if !profiles_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&profiles_dir).map_err(ProfileError::Io)?;

        for entry in entries {
            let entry = entry.map_err(ProfileError::Io)?;
            let path = entry.path();

            // 只处理 .toml 文件
            if path.extension().map(|e| e == "toml").unwrap_or(false) {
                match Profile::from_file(&path) {
                    Ok(profile) => {
                        self.profiles.insert(profile.name.clone(), profile);
                    }
                    Err(e) => {
                        eprintln!("警告: 加载 Profile {:?} 失败: {}", path, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取当前激活的配置
    ///
    /// 如果有激活的 Profile，将 Profile 的覆盖项合并到主配置。
    /// 否则返回主配置。
    pub fn get_active_config(&self) -> Config {
        if let Some(name) = &self.active_profile {
            if let Some(profile) = self.profiles.get(name) {
                return self.merge_profile(profile);
            }
        }
        (*self.base_config).clone()
    }

    /// 将 Profile 合并到主配置
    fn merge_profile(&self, profile: &Profile) -> Config {
        let mut config = (*self.base_config).clone();

        // 覆盖默认配置
        if let Some(model) = &profile.model {
            config.defaults.model = model.clone();
        }
        if let Some(tools_enabled) = profile.tools_enabled {
            config.defaults.tools_enabled = tools_enabled;
        }

        // 覆盖安全配置 - 修复：正确处理 false 值
        if let Some(yolo) = profile.yolo_mode {
            config.safety.yolo_mode = yolo;
        }
        if let Some(fast) = profile.fast_mode {
            config.safety.fast_mode = fast;
        }

        config
    }

    /// 切换到指定的 Profile
    pub fn switch_profile(&mut self, name: &str) -> Result<(), ProfileError> {
        if self.profiles.contains_key(name) {
            self.active_profile = Some(name.to_string());
            Ok(())
        } else {
            Err(ProfileError::NotFound(name.to_string()))
        }
    }

    /// 清除当前 Profile，使用主配置
    pub fn clear_profile(&mut self) {
        self.active_profile = None;
    }

    /// 列出所有 Profile 名称
    pub fn list_profiles(&self) -> Vec<&str> {
        self.profiles.keys().map(|s| s.as_str()).collect()
    }

    /// 获取当前激活的 Profile 名称
    pub fn get_active_profile_name(&self) -> Option<&str> {
        self.active_profile.as_deref()
    }

    /// 获取指定的 Profile
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// 添加新的 Profile
    pub fn add_profile(&mut self, profile: Profile) -> Result<(), ProfileError> {
        let name = profile.name.clone();

        // 保存到文件
        let path = super::config_dir().join("profiles").join(format!("{}.toml", name));
        profile.save(&path)?;

        // 添加到内存
        self.profiles.insert(name, profile);

        Ok(())
    }

    /// 删除 Profile
    pub fn remove_profile(&mut self, name: &str) -> Result<(), ProfileError> {
        if self.active_profile.as_deref() == Some(name) {
            self.active_profile = None;
        }

        // 删除文件
        let path = super::config_dir().join("profiles").join(format!("{}.toml", name));
        if path.exists() {
            fs::remove_file(&path).map_err(ProfileError::Io)?;
        }

        // 从内存移除
        self.profiles.remove(name);

        Ok(())
    }

    /// 更新 Profile
    pub fn update_profile(&mut self, profile: Profile) -> Result<(), ProfileError> {
        let name = profile.name.clone();

        // 保存到文件
        let path = super::config_dir().join("profiles").join(format!("{}.toml", name));
        profile.save(&path)?;

        // 更新内存
        self.profiles.insert(name, profile);

        Ok(())
    }

    /// 获取 Profile 数量
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }
}

/// Profile 错误类型
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Profile 不存在: {0}")]
    NotFound(String),
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("解析错误: {0}")]
    Parse(String),
    #[error("序列化错误: {0}")]
    Serialize(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("test")
            .with_description("测试配置")
            .with_model("anthropic/claude-sonnet-4-6")
            .with_yolo(true);

        assert_eq!(profile.name, "test");
        assert_eq!(profile.description, Some("测试配置".to_string()));
        assert_eq!(profile.model, Some("anthropic/claude-sonnet-4-6".to_string()));
        assert_eq!(profile.yolo_mode, Some(true));
    }

    #[test]
    fn test_profile_manager() {
        let base_config = Arc::new(Config::default());
        let mut manager = ProfileManager::new(base_config);

        let profile = Profile::new("work")
            .with_model("anthropic/claude-sonnet-4-6")
            .with_yolo(true);

        manager.profiles.insert("work".to_string(), profile);

        assert!(manager.switch_profile("work").is_ok());
        assert_eq!(manager.get_active_profile_name(), Some("work"));

        let config = manager.get_active_config();
        assert_eq!(config.defaults.model, "anthropic/claude-sonnet-4-6");
        assert!(config.safety.yolo_mode);
    }

    #[test]
    fn test_profile_not_found() {
        let base_config = Arc::new(Config::default());
        let mut manager = ProfileManager::new(base_config);

        let result = manager.switch_profile("nonexistent");
        assert!(result.is_err());
    }
}
