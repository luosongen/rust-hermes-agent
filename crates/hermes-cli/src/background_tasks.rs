//! 后台任务管理
//!
//! 支持 /background 命令，在后台异步执行 Agent 任务。

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// 后台任务状态
#[derive(Debug, Clone)]
pub enum TaskStatus {
    /// 运行中
    Running,
    /// 已完成
    Completed { result: String },
    /// 失败
    Failed { error: String },
}

/// 后台任务
#[derive(Debug, Clone)]
pub struct BackgroundTask {
    /// 任务 ID
    pub id: String,
    /// 用户输入的提示词
    pub prompt: String,
    /// 当前状态
    pub status: TaskStatus,
}

/// 后台任务管理器
///
/// 线程安全的 HashMap，跟踪所有后台任务的执行状态。
pub struct BackgroundTaskManager {
    tasks: RwLock<HashMap<String, BackgroundTask>>,
}

impl BackgroundTaskManager {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// 注册新任务（状态为 Running）
    pub fn register(&self, id: String, prompt: String) {
        let mut tasks = self.tasks.write();
        tasks.insert(
            id.clone(),
            BackgroundTask {
                id,
                prompt,
                status: TaskStatus::Running,
            },
        );
    }

    /// 标记任务完成
    pub fn complete(&self, id: &str, result: String) {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.get_mut(id) {
            task.status = TaskStatus::Completed { result };
        }
    }

    /// 标记任务失败
    pub fn fail(&self, id: &str, error: String) {
        let mut tasks = self.tasks.write();
        if let Some(task) = tasks.get_mut(id) {
            task.status = TaskStatus::Failed { error };
        }
    }

    /// 获取所有已完成的/失败的任务并清除
    ///
    /// REPL 循环每次 prompt 前调用，显示完成的任务。
    pub fn get_completed_and_clear(&self) -> Vec<BackgroundTask> {
        let mut tasks = self.tasks.write();
        let completed: Vec<BackgroundTask> = tasks
            .iter()
            .filter(|(_id, t)| !matches!(t.status, TaskStatus::Running))
            .map(|(_id, t)| t.clone())
            .collect();
        for task in &completed {
            tasks.remove(&task.id);
        }
        completed
    }

    /// 生成新的任务 ID
    pub fn generate_id() -> String {
        let ts = chrono::Local::now().format("%H%M%S");
        let uuid_suffix = uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("0")
            .to_string();
        format!("bg_{}_{}", ts, uuid_suffix)
    }
}

impl Default for BackgroundTaskManager {
    fn default() -> Self {
        Self::new()
    }
}
