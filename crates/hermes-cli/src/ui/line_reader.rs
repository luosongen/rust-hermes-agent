//! 异步 readline 封装

use rustyline::{config::Config, Editor};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 异步 readline 封装，提供行编辑和历史功能
pub struct LineReader {
    editor: Arc<Mutex<Editor<(), rustyline::history::FileHistory>>>,
}

impl LineReader {
    /// 创建新的 LineReader
    pub fn new(history_file: Option<&str>) -> Self {
        let config = Config::default();
        let mut editor = Editor::with_config(config).expect("Failed to create editor");
        if let Some(path) = history_file {
            let _ = editor.load_history(path).ok();
        }
        Self {
            editor: Arc::new(Mutex::new(editor)),
        }
    }

    /// 异步读取一行输入
    pub async fn read_line(&self, prompt: &str) -> Result<String, std::io::Error> {
        let editor = self.editor.clone();
        let prompt = prompt.to_string();

        // 在阻塞线程中运行 rustyline（因为它需要同步 IO）
        tokio::task::spawn_blocking(move || {
            let mut editor = editor.blocking_lock();
            editor.readline(&prompt)
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}