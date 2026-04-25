//! Multi-instance Profiles — 多实例配置文件管理

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Profile 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: Option<String>,
    pub config: ProfileConfig,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub default_model: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub custom_settings: HashMap<String, serde_json::Value>,
}

impl ProfileConfig {
    pub fn new() -> Self {
        Self {
            default_model: None,
            api_key: None,
            temperature: None,
            max_tokens: None,
            custom_settings: HashMap::new(),
        }
    }
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Profile 管理器
pub struct ProfileManager {
    profiles: Arc<RwLock<HashMap<String, Profile>>>,
    active_profile: Arc<RwLock<Option<String>>>,
    config_dir: PathBuf,
}

impl ProfileManager {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            profiles: Arc::new(RwLock::new(HashMap::new())),
            active_profile: Arc::new(RwLock::new(None)),
            config_dir,
        }
    }

    /// 创建新 Profile
    pub async fn create(&self, name: &str, config: ProfileConfig) -> Result<Profile, ProfileError> {
        let mut profiles = self.profiles.write().await;
        if profiles.contains_key(name) {
            return Err(ProfileError::AlreadyExists(name.to_string()));
        }

        let profile = Profile {
            name: name.to_string(),
            description: None,
            config,
            is_active: false,
        };

        profiles.insert(name.to_string(), profile.clone());
        Ok(profile)
    }

    /// 获取 Profile
    pub async fn get(&self, name: &str) -> Option<Profile> {
        let profiles = self.profiles.read().await;
        profiles.get(name).cloned()
    }

    /// 列出所有 Profiles
    pub async fn list(&self) -> Vec<Profile> {
        let profiles = self.profiles.read().await;
        profiles.values().cloned().collect()
    }

    /// 切换活动 Profile
    pub async fn activate(&self, name: &str) -> Result<(), ProfileError> {
        let mut profiles = self.profiles.write().await;
        if !profiles.contains_key(name) {
            return Err(ProfileError::NotFound(name.to_string()));
        }

        // 取消激活所有
        for profile in profiles.values_mut() {
            profile.is_active = false;
        }

        // 激活指定
        if let Some(profile) = profiles.get_mut(name) {
            profile.is_active = true;
        }

        *self.active_profile.write().await = Some(name.to_string());
        Ok(())
    }

    /// 获取当前活动的 Profile
    pub async fn active(&self) -> Option<Profile> {
        let active_name = self.active_profile.read().await.clone()?;
        let profiles = self.profiles.read().await;
        profiles.get(&active_name).cloned()
    }

    /// 更新 Profile 配置
    pub async fn update(&self, name: &str, config: ProfileConfig) -> Result<(), ProfileError> {
        let mut profiles = self.profiles.write().await;
        if let Some(profile) = profiles.get_mut(name) {
            profile.config = config;
            Ok(())
        } else {
            Err(ProfileError::NotFound(name.to_string()))
        }
    }

    /// 删除 Profile
    pub async fn delete(&mut self, name: &str) -> Result<(), ProfileError> {
        let mut profiles = self.profiles.write().await;
        if profiles.remove(name).is_some() {
            // 如果删除的是活动 profile，清除活动状态
            if self.active_profile.read().await.as_ref() == Some(&name.to_string()) {
                *self.active_profile.write().await = None;
            }
            Ok(())
        } else {
            Err(ProfileError::NotFound(name.to_string()))
        }
    }

    /// 从文件加载所有 Profiles
    pub async fn load_from_dir(&mut self) -> Result<(), ProfileError> {
        let dir = self.config_dir.join("profiles");
        if !dir.exists() {
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(&dir).await.map_err(|e| ProfileError::Io(e.to_string()))?;
        let mut profiles = self.profiles.write().await;

        while let Some(entry) = entries.next_entry().await.map_err(|e| ProfileError::Io(e.to_string()))? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "toml") {
                let content = tokio::fs::read_to_string(&path).await.map_err(|e| ProfileError::Io(e.to_string()))?;
                if let Ok(profile) = toml::from_str::<Profile>(&content) {
                    profiles.insert(profile.name.clone(), profile);
                }
            }
        }

        Ok(())
    }

    /// 保存所有 Profiles 到文件
    pub async fn save_all(&self) -> Result<(), ProfileError> {
        let dir = self.config_dir.join("profiles");
        tokio::fs::create_dir_all(&dir).await.map_err(|e| ProfileError::Io(e.to_string()))?;

        let profiles = self.profiles.read().await;
        for (name, profile) in profiles.iter() {
            let path = dir.join(format!("{}.toml", name));
            let content = toml::to_string_pretty(profile).map_err(|e| ProfileError::Serialize(e.to_string()))?;
            tokio::fs::write(&path, content).await.map_err(|e| ProfileError::Io(e.to_string()))?;
        }

        Ok(())
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new(PathBuf::from("~/.config/hermes-agent".to_string()))
    }
}

/// Profile 相关错误
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Profile not found: {0}")]
    NotFound(String),
    #[error("Profile already exists: {0}")]
    AlreadyExists(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Serialization error: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_profile() {
        let manager = ProfileManager::new(PathBuf::from("/tmp/test_profiles"));
        let config = ProfileConfig::new();
        let profile = manager.create("test", config).await.unwrap();
        assert_eq!(profile.name, "test");
    }

    #[tokio::test]
    async fn test_activate_profile() {
        let manager = ProfileManager::new(PathBuf::from("/tmp/test_profiles"));
        manager.create("test", ProfileConfig::new()).await.unwrap();
        manager.activate("test").await.unwrap();
        let active = manager.active().await.unwrap();
        assert_eq!(active.name, "test");
        assert!(active.is_active);
    }

    #[tokio::test]
    async fn test_delete_profile() {
        let mut manager = ProfileManager::new(PathBuf::from("/tmp/test_profiles"));
        manager.create("test", ProfileConfig::new()).await.unwrap();
        manager.delete("test").await.unwrap();
        assert!(manager.get("test").await.is_none());
    }
}