//! 异步 readline 封装

use crate::ui::completer::SlashCommandCompleter;
use rustyline::completion::{Completer, Pair};
use rustyline::config::Config;
use rustyline::highlight::Highlighter;
use rustyline::history::FileHistory;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Editor, Context};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 异步 readline 封装，提供行编辑和历史功能
pub struct LineReader {
    editor: Arc<Mutex<Editor<LineReaderAdapter, FileHistory>>>,
}

/// LineReader 的 Completer 适配器
pub struct LineReaderAdapter {
    inner: SlashCommandCompleter,
}

impl LineReader {
    /// 创建新的 LineReader
    pub fn new(history_file: Option<&str>) -> Self {
        let config = Config::default();
        let completer = SlashCommandCompleter::new();
        let adapter = LineReaderAdapter {
            inner: completer,
        };
        let mut editor = Editor::with_config(config).expect("Failed to create editor");
        if let Some(path) = history_file {
            let _ = editor.load_history(path).ok();
        }
        editor.set_helper(Some(adapter));
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

impl Completer for LineReaderAdapter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        // 找到需要补全的起始位置（从 pos 向前找到非单词字符）
        let start = line[..pos]
            .rfind(|c: char| !c.is_alphanumeric() && c != '/')
            .map(|i| i + 1)
            .unwrap_or(0);

        let prefix = &line[start..pos];

        // 使用内部 completer 生成补全
        let completions = self.inner.complete(prefix);

        let pairs: Vec<Pair> = completions
            .into_iter()
            .map(|s| Pair {
                display: s.clone(),
                replacement: s,
            })
            .collect();

        Ok((start, pairs))
    }
}

// 实现其他必须的 trait，使用空实现
impl Highlighter for LineReaderAdapter {}
impl Validator for LineReaderAdapter {}
impl Hinter for LineReaderAdapter {
    type Hint = String;
}

// 显式实现 Helper trait（它要求 Completer + Hinter + Highlighter + Validator）
impl rustyline::Helper for LineReaderAdapter {}