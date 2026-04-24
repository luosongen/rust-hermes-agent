//! Client cache — 按 (provider, model) 缓存客户端实例

use std::collections::HashMap;
use std::sync::Mutex;

use crate::adapters::ClientAdapter;

/// 客户端缓存
///
/// 按 (provider, model) 键值缓存 Box<dyn ClientAdapter>，避免重复创建。
pub struct ClientCache {
    cache: Mutex<HashMap<(String, String), Box<dyn ClientAdapter>>>,
}

impl ClientCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// 获取或创建客户端
    pub fn get_or_insert<F>(
        &self,
        key: (String, String),
        factory: F,
    ) -> Result<(), String>
    where
        F: FnOnce() -> Result<Box<dyn ClientAdapter>, String>,
    {
        let mut cache = self.cache.lock().map_err(|e| e.to_string())?;
        if cache.contains_key(&key) {
            return Ok(());
        }
        let client = factory()?;
        cache.insert(key, client);
        Ok(())
    }

    /// 清空缓存
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// 缓存条目数量
    pub fn len(&self) -> usize {
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ClientCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use hermes_core::{ChatRequest, ChatResponse, ProviderError};

    struct MockAdapter {
        name: &'static str,
    }

    #[async_trait]
    impl ClientAdapter for MockAdapter {
        fn provider_name(&self) -> &str {
            self.name
        }
        fn supported_models(&self) -> Vec<hermes_core::ModelId> {
            vec![]
        }
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, ProviderError> {
            Err(ProviderError::Api("mock".into()))
        }
    }

    #[test]
    fn test_cache_miss_then_insert() {
        let cache = ClientCache::new();
        assert!(cache.is_empty());

        let result = cache.get_or_insert(
            ("openai".into(), "gpt-4o".into()),
            || Ok(Box::new(MockAdapter { name: "openai" })),
        );
        assert!(result.is_ok());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_hit_no_duplicate_insert() {
        let cache = ClientCache::new();

        cache
            .get_or_insert(
                ("openai".into(), "gpt-4o".into()),
                || Ok(Box::new(MockAdapter { name: "openai" })),
            )
            .unwrap();

        // Second insert with same key should be no-op
        cache
            .get_or_insert(
                ("openai".into(), "gpt-4o".into()),
                || panic!("should not be called"),
            )
            .unwrap();

        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_multiple_providers() {
        let cache = ClientCache::new();

        cache
            .get_or_insert(
                ("openai".into(), "gpt-4o".into()),
                || Ok(Box::new(MockAdapter { name: "openai" })),
            )
            .unwrap();

        cache
            .get_or_insert(
                ("anthropic".into(), "claude-4".into()),
                || Ok(Box::new(MockAdapter { name: "anthropic" })),
            )
            .unwrap();

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_clear() {
        let cache = ClientCache::new();

        cache
            .get_or_insert(
                ("openai".into(), "gpt-4o".into()),
                || Ok(Box::new(MockAdapter { name: "openai" })),
            )
            .unwrap();

        cache.clear();
        assert!(cache.is_empty());
    }
}
