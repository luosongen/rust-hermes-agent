//! MultilineEditor — 多行输入编辑

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 行编辑状态
#[derive(Debug, Clone)]
pub enum EditState {
    /// 普通模式
    Normal,
    /// 多行模式
    Multiline,
    /// 草稿模式 (用于编辑)
    Draft,
}

/// 多行输入编辑器
pub struct MultilineEditor {
    /// 当前输入缓冲区
    buffer: Arc<RwLock<String>>,
    /// 编辑状态
    state: Arc<RwLock<EditState>>,
    /// 多行内容
    lines: Arc<RwLock<Vec<String>>>,
    /// 历史记录
    history: Arc<RwLock<VecDeque<String>>>,
    /// 最大历史记录数
    max_history: usize,
    /// 缩进字符串
    indent_str: String,
}

impl MultilineEditor {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(RwLock::new(String::new())),
            state: Arc::new(RwLock::new(EditState::Normal)),
            lines: Arc::new(RwLock::new(Vec::new())),
            history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            max_history: 100,
            indent_str: "  ".to_string(), // 2空格缩进
        }
    }

    /// 获取当前状态
    pub async fn state(&self) -> EditState {
        self.state.read().await.clone()
    }

    /// 切换到多行模式
    pub async fn enter_multiline(&self) {
        *self.state.write().await = EditState::Multiline;
        self.lines.write().await.clear();
    }

    /// 退出多行模式
    pub async fn exit_multiline(&self) {
        *self.state.write().await = EditState::Normal;
    }

    /// 添加一行
    pub async fn add_line(&self, line: String) {
        self.lines.write().await.push(line);
    }

    /// 获取当前多行内容
    pub async fn content(&self) -> String {
        let lines = self.lines.read().await;
        lines.join("\n")
    }

    /// 获取带缩进的内容
    pub async fn indented_content(&self) -> String {
        let lines = self.lines.read().await;
        lines
            .iter()
            .map(|line| format!("{}{}", self.indent_str, line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 检查是否为空
    pub async fn is_empty(&self) -> bool {
        self.lines.read().await.is_empty()
    }

    /// 清除内容
    pub async fn clear(&self) {
        self.lines.write().await.clear();
        self.buffer.write().await.clear();
    }

    /// 提交多行输入
    pub async fn submit(&self) -> Option<String> {
        let content = self.content().await;
        if !content.is_empty() {
            // 添加到历史
            self.push_history(content.clone()).await;
            self.clear().await;
            *self.state.write().await = EditState::Normal;
            Some(content)
        } else {
            None
        }
    }

    /// 从历史记录恢复
    pub async fn recall(&self, index: usize) -> Option<String> {
        let history = self.history.read().await;
        if index < history.len() {
            Some(history[index].clone())
        } else {
            None
        }
    }

    /// 推送历史
    async fn push_history(&self, entry: String) {
        let mut history = self.history.write().await;
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(entry);
    }

    /// 插入文本到缓冲区
    pub async fn insert(&self, text: &str) {
        self.buffer.write().await.push_str(text);
    }

    /// 获取缓冲区内容
    pub async fn buffer(&self) -> String {
        self.buffer.read().await.clone()
    }

    /// 清除缓冲区
    pub async fn clear_buffer(&self) {
        self.buffer.write().await.clear();
    }

    /// 回退一行 (用于退格)
    pub async fn undo_line(&self) -> Option<String> {
        self.lines.write().await.pop()
    }

    /// 获取行数
    pub async fn line_count(&self) -> usize {
        self.lines.read().await.len()
    }
}

impl Default for MultilineEditor {
    fn default() -> Self {
        Self::new()
    }
}

/// 辅助函数：检测是否需要多行输入
pub fn needs_multiline_input(input: &str) -> bool {
    // 检查是否有未闭合的括号、引号等
    let paren_count = input.matches('(').count() - input.matches(')').count();
    let bracket_count = input.matches('[').count() - input.matches(']').count();
    let brace_count = input.matches('{').count() - input.matches('}').count();
    let quote_count = input.matches('"').count() - input.matches('"').count() % 2 == 0;

    paren_count > 0 || bracket_count > 0 || brace_count > 0 || !quote_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_multiline_mode() {
        let editor = MultilineEditor::new();
        assert!(matches!(editor.state().await, EditState::Normal));

        editor.enter_multiline().await;
        assert!(matches!(editor.state().await, EditState::Multiline));

        editor.add_line("line 1".to_string()).await;
        editor.add_line("line 2".to_string()).await;

        let content = editor.content().await;
        assert!(content.contains("line 1"));
        assert!(content.contains("line 2"));
    }

    #[tokio::test]
    async fn test_submit() {
        let editor = MultilineEditor::new();
        editor.enter_multiline().await;
        editor.add_line("test content".to_string()).await;

        let submitted = editor.submit().await;
        assert!(submitted.is_some());
        assert!(editor.is_empty().await);
    }

    #[tokio::test]
    async fn test_indent() {
        let editor = MultilineEditor::new();
        editor.enter_multiline().await;
        editor.add_line("code".to_string()).await;

        let indented = editor.indented_content().await;
        assert!(indented.starts_with("  "));
    }

    #[test]
    fn test_needs_multiline() {
        assert!(needs_multiline_input("fn foo("));
        assert!(!needs_multiline_input("simple text"));
    }
}