//! CommandHistory — 命令历史管理

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 最大历史条目数
const MAX_HISTORY_SIZE: usize = 1000;

/// 命令历史记录
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub command: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl HistoryEntry {
    pub fn new(command: String) -> Self {
        Self {
            command,
            timestamp: chrono::Utc::now(),
        }
    }
}

/// 命令历史管理器
pub struct CommandHistory {
    entries: Arc<RwLock<Vec<HistoryEntry>>>,
    history_file: Option<PathBuf>,
}

impl CommandHistory {
    /// 创建新的历史管理器
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
            history_file: None,
        }
    }

    /// 从文件加载历史记录
    pub async fn load(&mut self, path: PathBuf) -> std::io::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let lines: Vec<String> = content.lines().map(String::from).filter(|s| !s.is_empty()).collect();

        let mut entries = self.entries.write().await;
        for line in lines {
            entries.push(HistoryEntry::new(line));
        }

        // 保持最大条目数限制
        while entries.len() > MAX_HISTORY_SIZE {
            entries.remove(0);
        }

        self.history_file = Some(path);
        Ok(())
    }

    /// 保存历史到文件
    pub async fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.history_file else {
            return Ok(());
        };

        let entries = self.entries.read().await;
        let content: String = entries
            .iter()
            .map(|e| e.command.clone())
            .collect::<Vec<_>>()
            .join("\n");

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, content).await?;

        Ok(())
    }

    /// 添加命令到历史
    pub async fn push(&self, command: String) {
        let mut entries = self.entries.write().await;

        // 避免连续重复
        if let Some(last) = entries.last() {
            if last.command == command {
                return;
            }
        }

        entries.push(HistoryEntry::new(command));

        // 限制大小
        while entries.len() > MAX_HISTORY_SIZE {
            entries.remove(0);
        }
    }

    /// 获取历史条目
    pub async fn entries(&self) -> Vec<String> {
        self.entries.read().await.iter().map(|e| e.command.clone()).collect()
    }

    /// 搜索历史
    pub async fn search(&self, prefix: &str) -> Vec<String> {
        self.entries
            .read()
            .await
            .iter()
            .filter(|e| e.command.starts_with(prefix))
            .map(|e| e.command.clone())
            .collect()
    }

    /// 清除历史
    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_push_and_entries() {
        let history = CommandHistory::new();
        history.push("test command".to_string()).await;
        let entries = history.entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "test command");
    }

    #[tokio::test]
    async fn test_no_duplicate_consecutive() {
        let history = CommandHistory::new();
        history.push("test".to_string()).await;
        history.push("test".to_string()).await;
        let entries = history.entries().await;
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn test_search() {
        let history = CommandHistory::new();
        history.push("hello world".to_string()).await;
        history.push("hello there".to_string()).await;
        history.push("goodbye".to_string()).await;

        let results = history.search("hello").await;
        assert_eq!(results.len(), 2);
    }
}