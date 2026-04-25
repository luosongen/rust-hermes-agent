//! SubAgent — 子代理实现

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

use crate::error::AgentError;

/// 子代理状态
#[derive(Debug, Clone)]
pub enum SubAgentState {
    Idle,
    Running,
    Suspended,
    Terminated,
}

/// 子代理标识符
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SubAgentId(pub String);

impl SubAgentId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SubAgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 子代理元数据
#[derive(Debug, Clone)]
pub struct SubAgentMetadata {
    pub id: SubAgentId,
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub parent_id: Option<SubAgentId>,
    pub capabilities: Vec<String>,
}

impl SubAgentMetadata {
    pub fn new(id: SubAgentId, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            description: None,
            created_at: chrono::Utc::now(),
            parent_id: None,
            capabilities: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_parent(mut self, parent_id: SubAgentId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// 子代理 trait — 可被主代理委托任务
#[async_trait]
pub trait SubAgent: Send + Sync {
    /// 获取子代理元数据
    fn metadata(&self) -> &SubAgentMetadata;

    /// 执行任务
    async fn run(&mut self, task: &str) -> Result<String, AgentError>;

    /// 获取当前状态
    fn state(&self) -> SubAgentState;

    /// 暂停执行
    async fn suspend(&mut self);

    /// 恢复执行
    async fn resume(&mut self);

    /// 终止执行
    async fn terminate(&mut self);
}

/// 子代理管理器
pub struct SubAgentManager {
    agents: Arc<RwLock<HashMap<SubAgentId, Box<dyn SubAgent>>>>,
    max_agents: usize,
}

impl SubAgentManager {
    pub fn new(max_agents: usize) -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            max_agents,
        }
    }

    /// 注册子代理
    pub async fn register(&mut self, agent: Box<dyn SubAgent>) -> Result<(), AgentError> {
        let mut agents = self.agents.write().await;
        if agents.len() >= self.max_agents {
            return Err(AgentError::Other(format!(
                "Maximum agents {} reached",
                self.max_agents
            ).into()));
        }
        let id = agent.metadata().id.clone();
        agents.insert(id, agent);
        Ok(())
    }

    /// 获取子代理
    pub async fn get(&self, id: &SubAgentId) -> Option<Box<dyn SubAgent>> {
        let agents = self.agents.read().await;
        agents.get(id).cloned()
    }

    /// 移除子代理
    pub async fn remove(&mut self, id: &SubAgentId) -> Option<Box<dyn SubAgent>> {
        let mut agents = self.agents.write().await;
        agents.remove(id)
    }

    /// 列出所有子代理
    pub async fn list(&self) -> Vec<SubAgentMetadata> {
        let agents = self.agents.read().await;
        agents.values().map(|a| a.metadata().clone()).collect()
    }

    /// 获取运行中的子代理数量
    pub async fn running_count(&self) -> usize {
        let agents = self.agents.read().await;
        agents.values().filter(|a| matches!(a.state(), SubAgentState::Running)).count()
    }

    /// 清理已终止的子代理
    pub async fn cleanup(&mut self) {
        let mut agents = self.agents.write().await;
        agents.retain(|_, a| !matches!(a.state(), SubAgentState::Terminated));
    }
}

impl Default for SubAgentManager {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_id() {
        let id = SubAgentId::new("test-agent");
        assert_eq!(id.as_str(), "test-agent");
        assert_eq!(id.to_string(), "test-agent");
    }

    #[test]
    fn test_subagent_metadata() {
        let meta = SubAgentMetadata::new(
            SubAgentId::new("agent-1"),
            "Test Agent",
        )
        .with_description("A test agent")
        .with_capabilities(vec!["coding".to_string(), "analysis".to_string()]);

        assert_eq!(meta.name, "Test Agent");
        assert!(meta.description.is_some());
        assert_eq!(meta.capabilities.len(), 2);
    }
}