//! TUI App — 多面板终端用户界面
//!
//! 基于 ratatui 实现的多面板 TUI 界面。

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, Stdout};
use std::time::Duration;

/// TUI 应用状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    /// 聊天模式
    Chat,
    /// 工具视图模式
    ToolView,
    /// 帮助模式
    Help,
}

/// 最大消息数量（防止内存无限增长）
const MAX_MESSAGES: usize = 1000;

/// TUI 应用
pub struct TuiApp {
    /// 应用模式
    pub mode: AppMode,
    /// 当前模型
    pub model: String,
    /// 当前配置名称
    pub profile: String,
    /// Token 使用统计
    pub token_usage: (u64, u64, u64), // (prompt, completion, total)
    /// YOLO 模式
    pub yolo_mode: bool,
    /// Fast 模式
    pub fast_mode: bool,
    /// 聊天消息历史
    pub messages: Vec<ChatMessage>,
    /// 当前输入
    pub input: String,
    /// 输入光标位置
    pub input_cursor: usize,
    /// 当前工具状态
    pub tool_status: Option<ToolStatus>,
    /// 是否应该退出
    pub should_exit: bool,
    /// 滚动偏移
    pub scroll_offset: usize,
}

/// 聊天消息
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: String,
}

/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

/// 工具状态
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: String,
    pub status: ToolExecutionStatus,
    pub output: String,
}

/// 工具执行状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolExecutionStatus {
    Idle,
    Running,
    Success,
    Failed,
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiApp {
    /// 创建新的 TUI 应用
    pub fn new() -> Self {
        Self {
            mode: AppMode::Chat,
            model: "openai/gpt-4o".to_string(),
            profile: "default".to_string(),
            token_usage: (0, 0, 0),
            yolo_mode: false,
            fast_mode: false,
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            tool_status: None,
            should_exit: false,
            scroll_offset: 0,
        }
    }

    /// 运行 TUI 主循环
    pub fn run(&mut self) -> Result<(), io::Error> {
        // 设置终端
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // 主循环
        let res = self.run_loop(&mut terminal);

        // 恢复终端
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        res
    }

    /// 主循环
    fn run_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), io::Error> {
        loop {
            // 绘制
            terminal.draw(|f| self.render(f))?;

            // 处理事件
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key);
                }
            }

            if self.should_exit {
                break;
            }
        }

        Ok(())
    }

    /// 处理键盘事件
    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_exit = true;
            }
            KeyCode::Esc => {
                if self.mode == AppMode::Help {
                    self.mode = AppMode::Chat;
                } else {
                    self.should_exit = true;
                }
            }
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    // 发送消息
                    self.messages.push(ChatMessage {
                        role: MessageRole::User,
                        content: self.input.clone(),
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                    });
                    self.input.clear();
                    self.input_cursor = 0;
                    // 滚动到底部
                    self.scroll_offset = self.messages.len().saturating_sub(1);
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input.remove(self.input_cursor - 1);
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input.len() {
                    self.input.remove(self.input_cursor);
                }
            }
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor += 1;
                }
            }
            KeyCode::Up => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            KeyCode::Down => {
                if self.scroll_offset < self.messages.len().saturating_sub(1) {
                    self.scroll_offset += 1;
                }
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + 10).min(self.messages.len().saturating_sub(1));
            }
            KeyCode::F(1) => {
                self.mode = if self.mode == AppMode::Help {
                    AppMode::Chat
                } else {
                    AppMode::Help
                };
            }
            _ => {}
        }
    }

    /// 渲染界面
    fn render(&self, f: &mut Frame) {
        // 创建布局
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),    // 聊天区域
                Constraint::Length(3),  // 工具面板
                Constraint::Length(3),  // 输入框
                Constraint::Length(1),  // 状态栏
            ])
            .split(f.size());

        // 渲染聊天视图
        self.render_chat_view(f, chunks[0]);

        // 渲染工具面板
        self.render_tool_panel(f, chunks[1]);

        // 渲染输入框
        self.render_input(f, chunks[2]);

        // 渲染状态栏
        self.render_status_bar(f, chunks[3]);
    }

    /// 渲染聊天视图
    fn render_chat_view(&self, f: &mut Frame, area: Rect) {
        let mut lines = Vec::new();

        for msg in &self.messages {
            let (prefix, color) = match msg.role {
                MessageRole::User => ("你", Color::Green),
                MessageRole::Assistant => ("AI", Color::Blue),
                MessageRole::System => ("系统", Color::Yellow),
                MessageRole::Tool => ("工具", Color::Magenta),
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("[{}] ", msg.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{}: ", prefix), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            ]));

            // 简单换行处理
            for line in msg.content.lines() {
                lines.push(Line::from(line.to_string()));
            }
            lines.push(Line::from(""));
        }

        let chat = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("聊天"))
            .wrap(Wrap { trim: false });

        f.render_widget(chat, area);
    }

    /// 渲染工具面板
    fn render_tool_panel(&self, f: &mut Frame, area: Rect) {
        let content = match &self.tool_status {
            Some(tool) => {
                let status_icon = match tool.status {
                    ToolExecutionStatus::Idle => "⚪",
                    ToolExecutionStatus::Running => "🔵",
                    ToolExecutionStatus::Success => "🟢",
                    ToolExecutionStatus::Failed => "🔴",
                };
                format!("{} {} {}", status_icon, tool.name, tool.output)
            }
            None => "准备就绪".to_string(),
        };

        let panel = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("工具"))
            .style(Style::default().fg(Color::White));

        f.render_widget(panel, area);
    }

    /// 渲染输入框
    fn render_input(&self, f: &mut Frame, area: Rect) {
        let input = Paragraph::new(self.input.as_str())
            .block(Block::default().borders(Borders::ALL).title("输入 (F1 帮助, Esc 退出)"))
            .style(Style::default().fg(Color::White));

        f.render_widget(input, area);

        // 显示光标
        #[cfg(not(target_os = "windows"))]
        {
            let cursor_x = area.x + self.input_cursor as u16 + 1;
            let cursor_y = area.y + 1;
            f.set_cursor(cursor_x, cursor_y);
        }
    }

    /// 渲染状态栏
    fn render_status_bar(&self, f: &mut Frame, area: Rect) {
        let mode_text = match self.mode {
            AppMode::Chat => "聊天",
            AppMode::ToolView => "工具",
            AppMode::Help => "帮助",
        };

        let yolo_text = if self.yolo_mode { " YOLO" } else { "" };
        let fast_text = if self.fast_mode { " FAST" } else { "" };

        let text = format!(
            "模型:{} | 配置:{} | Tokens:{}/{}/{} | 模式:{}{}{}",
            self.model,
            self.profile,
            self.token_usage.0,
            self.token_usage.1,
            self.token_usage.2,
            mode_text,
            yolo_text,
            fast_text
        );

        let status = Paragraph::new(text)
            .style(Style::default().bg(Color::Blue).fg(Color::White));

        f.render_widget(status, area);
    }

    /// 添加消息
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        // 限制消息数量，防止内存无限增长
        if self.messages.len() >= MAX_MESSAGES {
            self.messages.remove(0);
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
            }
        }

        self.messages.push(ChatMessage {
            role,
            content,
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });
    }

    /// 设置工具状态
    pub fn set_tool_status(&mut self, name: String, status: ToolExecutionStatus, output: String) {
        self.tool_status = Some(ToolStatus { name, status, output });
    }

    /// 更新 token 使用
    pub fn update_token_usage(&mut self, prompt: u64, completion: u64) {
        self.token_usage = (prompt, completion, prompt + completion);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = TuiApp::new();
        assert_eq!(app.mode, AppMode::Chat);
        assert!(!app.should_exit);
    }

    #[test]
    fn test_add_message() {
        let mut app = TuiApp::new();
        app.add_message(MessageRole::User, "Hello".to_string());
        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].content, "Hello");
    }

    #[test]
    fn test_token_usage() {
        let mut app = TuiApp::new();
        app.update_token_usage(100, 50);
        assert_eq!(app.token_usage, (100, 50, 150));
    }
}
