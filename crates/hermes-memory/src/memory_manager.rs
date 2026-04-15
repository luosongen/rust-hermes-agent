//! MemoryManager - 内存提供者协调器
//!
//! 协调内置内存提供者和外部插件提供者。

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// MemoryProvider trait - 内存提供者接口
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// 提供者名称
    fn name(&self) -> &str;

    /// 获取工具 schema（用于动态工具注册）
    fn get_tool_schemas(&self) -> Vec<serde_json::Value>;

    /// 构建系统提示词块
    fn system_prompt_block(&self) -> String;

    /// 预取相关记忆
    fn prefetch(&self, query: &str, session_id: &str) -> String;

    /// 队列预取（异步，后台更新）
    fn queue_prefetch(&self, query: &str, session_id: &str);

    /// 同步一轮对话
    fn sync_turn(&self, user_content: &str, assistant_content: &str, session_id: &str);

    /// 处理工具调用
    fn handle_tool_call(&self, tool_name: &str, args: serde_json::Value) -> Result<String, String>;
}

// =============================================================================
// MemoryManager
// =============================================================================

/// MemoryManager - 协调多个内存提供者
///
/// 内置提供者始终优先注册。只允许一个外部（非内置）提供者。
pub struct MemoryManager {
    providers: Vec<Arc<dyn MemoryProvider>>,
    tool_to_provider: HashMap<String, Arc<dyn MemoryProvider>>,
    has_external: bool,
}

impl Default for MemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryManager {
    /// 创建新的 MemoryManager
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            tool_to_provider: HashMap::new(),
            has_external: false,
        }
    }

    /// 注册内存提供者
    ///
    /// 内置提供者（name == "builtin"）始终接受。
    /// 只允许一个外部（非内置）提供者 - 第二次注册将被拒绝。
    pub fn add_provider(&mut self, provider: Arc<dyn MemoryProvider>) -> Result<(), String> {
        let is_builtin = provider.name() == "builtin";

        if !is_builtin && self.has_external {
            return Err(format!(
                "External memory provider '{}' already registered. Only one external memory provider is allowed.",
                provider.name()
            ));
        }

        if !is_builtin {
            self.has_external = true;
        }

        // 索引工具名称到提供者
        for schema in provider.get_tool_schemas() {
            if let Some(tool_name) = schema.get("name").and_then(|n| n.as_str()) {
                if !self.tool_to_provider.contains_key(tool_name) {
                    self.tool_to_provider.insert(tool_name.to_string(), Arc::clone(&provider));
                }
            }
        }

        self.providers.push(provider);
        Ok(())
    }

    /// 获取所有注册提供者
    pub fn providers(&self) -> Vec<Arc<dyn MemoryProvider>> {
        self.providers.clone()
    }

    /// 构建系统提示词
    ///
    /// 收集所有提供者的系统提示词块，用提供者名称标注。
    pub fn build_system_prompt(&self) -> String {
        let mut blocks = Vec::new();
        for provider in &self.providers {
            let block = provider.system_prompt_block();
            if !block.trim().is_empty() {
                blocks.push(format!("[{}]\n{}", provider.name(), block));
            }
        }
        blocks.join("\n\n")
    }

    /// 预取所有相关记忆
    pub fn prefetch_all(&self, query: &str, session_id: &str) -> String {
        let mut parts = Vec::new();
        for provider in &self.providers {
            let result = provider.prefetch(query, session_id);
            if !result.trim().is_empty() {
                parts.push(result);
            }
        }
        parts.join("\n\n")
    }

    /// 队列预取所有提供者的相关记忆
    pub fn queue_prefetch_all(&self, query: &str, session_id: &str) {
        for provider in &self.providers {
            provider.queue_prefetch(query, session_id);
        }
    }

    /// 同步一轮对话到所有提供者
    pub fn sync_all(&self, user_content: &str, assistant_content: &str, session_id: &str) {
        for provider in &self.providers {
            provider.sync_turn(user_content, assistant_content, session_id);
        }
    }

    /// 获取所有工具 schema
    pub fn get_all_tool_schemas(&self) -> Vec<serde_json::Value> {
        let mut schemas = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for provider in &self.providers {
            for schema in provider.get_tool_schemas() {
                if let Some(name) = schema.get("name").and_then(|n| n.as_str()) {
                    if !seen.contains(name) {
                        seen.insert(name.to_string());
                        schemas.push(schema);
                    }
                }
            }
        }
        schemas
    }

    /// 获取所有工具名称
    pub fn get_all_tool_names(&self) -> std::collections::HashSet<String> {
        self.tool_to_provider.keys().cloned().collect()
    }

    /// 检查是否有某个工具
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tool_to_provider.contains_key(tool_name)
    }

    /// 处理工具调用
    pub fn handle_tool_call(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, String> {
        let provider = self.tool_to_provider.get(tool_name)
            .ok_or_else(|| format!("No memory provider handles tool '{}'", tool_name))?;

        provider.handle_tool_call(tool_name, args)
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Mock provider for testing
    struct MockMemoryProvider {
        name: String,
    }

    impl MockMemoryProvider {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    #[async_trait]
    impl MemoryProvider for MockMemoryProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn get_tool_schemas(&self) -> Vec<serde_json::Value> {
            vec![serde_json::json!({
                "name": format!("{}_tool", self.name),
                "description": "Test tool",
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            })]
        }

        fn system_prompt_block(&self) -> String {
            format!("Mock system prompt from {}", self.name)
        }

        fn prefetch(&self, query: &str, _session_id: &str) -> String {
            format!("Memory for '{}' from {}", query, self.name)
        }

        fn queue_prefetch(&self, _query: &str, _session_id: &str) {
            // No-op for testing
        }

        fn sync_turn(&self, user: &str, assistant: &str, _session_id: &str) {
            println!("Syncing: user={}, assistant={}", user, assistant);
        }

        fn handle_tool_call(&self, tool_name: &str, _args: serde_json::Value) -> Result<String, String> {
            Ok(format!("Result from {}", tool_name))
        }
    }

    #[test]
    fn test_add_builtin_provider() {
        let mut manager = MemoryManager::new();
        let provider = Arc::new(MockMemoryProvider::new("builtin"));
        let result = manager.add_provider(provider);
        assert!(result.is_ok());
        assert_eq!(manager.providers().len(), 1);
    }

    #[test]
    fn test_add_external_provider() {
        let mut manager = MemoryManager::new();
        let provider = Arc::new(MockMemoryProvider::new("custom"));
        let result = manager.add_provider(provider);
        assert!(result.is_ok());
        assert_eq!(manager.providers().len(), 1);
    }

    #[test]
    fn test_reject_second_external_provider() {
        let mut manager = MemoryManager::new();
        let builtin = Arc::new(MockMemoryProvider::new("builtin"));
        let _ = manager.add_provider(builtin);
        let external1 = Arc::new(MockMemoryProvider::new("external1"));
        let _ = manager.add_provider(external1);
        let external2 = Arc::new(MockMemoryProvider::new("external2"));
        let result = manager.add_provider(external2);
        assert!(result.is_err());
        assert_eq!(manager.providers().len(), 2);
    }

    #[test]
    fn test_build_system_prompt() {
        let mut manager = MemoryManager::new();
        let provider1 = Arc::new(MockMemoryProvider::new("builtin"));
        let provider2 = Arc::new(MockMemoryProvider::new("builtin2"));
        manager.add_provider(provider1).unwrap();
        manager.add_provider(provider2).unwrap();

        let prompt = manager.build_system_prompt();
        assert!(prompt.contains("builtin"));
        assert!(prompt.contains("builtin2"));
    }

    #[test]
    fn test_prefetch_all() {
        let mut manager = MemoryManager::new();
        let provider1 = Arc::new(MockMemoryProvider::new("builtin"));
        let provider2 = Arc::new(MockMemoryProvider::new("builtin2"));
        manager.add_provider(provider1).unwrap();
        manager.add_provider(provider2).unwrap();

        let result = manager.prefetch_all("test query", "session123");
        assert!(result.contains("builtin"));
        assert!(result.contains("builtin2"));
    }

    #[test]
    fn test_has_tool() {
        let mut manager = MemoryManager::new();
        let provider = Arc::new(MockMemoryProvider::new("test"));
        manager.add_provider(provider).unwrap();

        assert!(manager.has_tool("test_tool"));
        assert!(!manager.has_tool("nonexistent"));
    }
}
