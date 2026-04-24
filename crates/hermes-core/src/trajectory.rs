//! Trajectory Saver — 保存对话轨迹到 JSONL

use crate::{Content, Message};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 轨迹保存器
///
/// 将对话保存为 JSONL 格式，成功轨迹和失败轨迹分别存储。
pub struct TrajectorySaver {
    output_dir: PathBuf,
}

/// 轨迹条目（ShareGPT 格式）
#[derive(Debug, Serialize)]
struct TrajectoryEntry {
    model: String,
    completed: bool,
    timestamp: f64,
    messages: Vec<TrajectoryMessage>,
}

/// 轨迹消息（简化格式）
#[derive(Debug, Serialize)]
struct TrajectoryMessage {
    role: String,
    content: String,
}

impl From<&Message> for TrajectoryMessage {
    fn from(msg: &Message) -> Self {
        let content = match &msg.content {
            Content::Text(t) => t.clone(),
            Content::Image { url, .. } => format!("[image: {}]", url),
            Content::ToolResult { content, .. } => content.clone(),
        };
        Self {
            role: format!("{:?}", msg.role).to_lowercase(),
            content,
        }
    }
}

impl TrajectorySaver {
    /// 创建轨迹保存器
    ///
    /// `output_dir` — 输出目录（如 `~/.config/hermes-agent/trajectories`）
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        let output_dir = output_dir.into();
        // 确保目录存在
        if !output_dir.exists() {
            let _ = std::fs::create_dir_all(&output_dir);
        }
        Self { output_dir }
    }

    /// 保存轨迹
    ///
    /// `messages` — 对话消息列表
    /// `model` — 使用的模型名称
    /// `completed` — 是否成功完成
    pub fn save(
        &self,
        messages: &[Message],
        model: &str,
        completed: bool,
    ) -> Result<(), std::io::Error> {
        let filename = if completed {
            "trajectories.jsonl"
        } else {
            "failed_trajectories.jsonl"
        };

        let entry = TrajectoryEntry {
            model: model.to_string(),
            completed,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64(),
            messages: messages.iter().map(|m| m.into()).collect(),
        };

        let line = serde_json::to_string(&entry)?;
        let path = self.output_dir.join(filename);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        writeln!(file, "{}", line)?;
        file.flush()?;
        Ok(())
    }

    /// 获取输出目录
    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

impl Default for TrajectorySaver {
    fn default() -> Self {
        let dir = dirs::config_dir()
            .map(|p| p.join("hermes-agent").join("trajectories"))
            .unwrap_or_else(|| PathBuf::from("./trajectories"));
        Self::new(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_saves_to_file() {
        let temp_dir = std::env::temp_dir().join("hermes-test-trajectory");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let saver = TrajectorySaver::new(&temp_dir);

        let messages = vec![
            Message::system("You are a helpful assistant."),
            Message::user("Hello"),
            Message::assistant("Hi there!"),
        ];

        saver.save(&messages, "openai/gpt-4o", true).unwrap();

        let path = temp_dir.join("trajectories.jsonl");
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("gpt-4o"));
        assert!(content.contains("Hello"));
        assert!(content.contains("completed"));
    }

    #[test]
    fn test_failed_trajectory_separate_file() {
        let temp_dir = std::env::temp_dir().join("hermes-test-trajectory-fail");
        let _ = std::fs::remove_dir_all(&temp_dir);
        let saver = TrajectorySaver::new(&temp_dir);

        let messages = vec![Message::user("test")];
        saver.save(&messages, "anthropic/claude-4", false).unwrap();

        let path = temp_dir.join("failed_trajectories.jsonl");
        assert!(path.exists());

        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("claude-4"));
        assert!(content.contains("\"completed\":false"));
    }
}
