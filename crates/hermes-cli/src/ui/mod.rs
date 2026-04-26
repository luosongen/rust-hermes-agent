//! UI 模块 — 终端增强功能
//!
//! 提供 REPL 增强功能：
//! - MultilineEditor: 多行输入编辑
//! - SlashCommandCompleter: 斜杠命令自动补全
//! - StreamingOutput: 流式输出显示
//! - CommandHistory: 命令历史管理

pub mod line_reader;
pub mod multiline_editor;
pub mod completer;
pub mod streaming_output;
pub mod history;

pub use line_reader::LineReader;
pub use multiline_editor::MultilineEditor;
pub use completer::{SlashCommandCompleter, CommandMetadata};
pub use streaming_output::{StreamingOutput, OutputChunk};
pub use history::CommandHistory;
