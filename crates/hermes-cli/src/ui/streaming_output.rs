//! StreamingOutput — 流式输出显示

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 输出块类型
#[derive(Debug, Clone)]
pub enum OutputChunk {
    /// 文本内容
    Text(String),
    /// 工具调用开始
    ToolStart { name: String, id: String },
    /// 工具调用完成
    ToolEnd { id: String },
    /// 错误信息
    Error(String),
    /// 完成信号
    Done,
}

impl OutputChunk {
    pub fn text(s: impl Into<String>) -> Self {
        OutputChunk::Text(s.into())
    }

    pub fn error(s: impl Into<String>) -> Self {
        OutputChunk::Error(s.into())
    }
}

/// 流式输出处理器
pub struct StreamingOutput {
    enabled: Arc<AtomicBool>,
    buffer: std::sync::Mutex<String>,
}

impl StreamingOutput {
    pub fn new() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            buffer: std::sync::Mutex::new(String::new()),
        }
    }

    /// 启用流式输出
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    /// 禁用流式输出
    pub fn disable(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// 处理输出块
    pub fn write(&self, chunk: OutputChunk) {
        if !self.is_enabled() {
            return;
        }

        match chunk {
            OutputChunk::Text(text) => {
                print!("{}", text);
                // 刷新以实时显示
                print!("\n");
            }
            OutputChunk::ToolStart { name, id } => {
                println!("[工具调用: {} ({})]", name, id);
            }
            OutputChunk::ToolEnd { id: _ } => {
                println!("[工具调用完成]");
            }
            OutputChunk::Error(err) => {
                eprintln!("错误: {}", err);
            }
            OutputChunk::Done => {
                println!("[完成]");
            }
        }
        // 确保立即显示
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }

    /// 批量写入文本
    pub fn write_text(&self, text: &str) {
        if self.is_enabled() {
            print!("{}", text);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }

    /// 写入带前缀的文本行
    pub fn write_line(&self, prefix: &str, text: &str) {
        if self.is_enabled() {
            println!("{} {}", prefix, text);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }

    /// 清除当前行
    pub fn clear_line(&self) {
        print!("\r\x1b[K");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }

    /// 显示进度指示器
    pub fn show_progress(&self, message: &str) {
        if self.is_enabled() {
            self.clear_line();
            println!("⏳ {}", message);
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }

    /// 完成进度
    pub fn finish_progress(&self, message: &str) {
        if self.is_enabled() {
            self.clear_line();
            println!("✅ {}", message);
        }
    }

    /// 显示加载动画
    pub fn start_loading(&self, message: &str) {
        if !self.enabled.load(Ordering::SeqCst) {
            self.enabled.store(true, Ordering::SeqCst);
            let enabled = self.enabled.clone();
            let msg = message.to_string();
            tokio::spawn(async move {
                let spinner = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏";
                let mut i = 0;
                while enabled.load(Ordering::SeqCst) {
                    print!("\r{} {}{}\x1B[K", spinner.chars().nth(i % 10).unwrap_or(' '), msg, "...");
                    std::io::stdout().flush().ok();
                    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                    i += 1;
                }
                print!("\r\x1B[K");
                std::io::stdout().flush().ok();
            });
        }
    }

    /// 停止加载动画
    pub fn stop_loading(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }
}

impl Default for StreamingOutput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enabled_default() {
        let output = StreamingOutput::new();
        assert!(output.is_enabled());
    }

    #[test]
    fn test_enable_disable() {
        let output = StreamingOutput::new();
        output.disable();
        assert!(!output.is_enabled());
        output.enable();
        assert!(output.is_enabled());
    }

    #[test]
    fn test_output_chunk_variants() {
        let text = OutputChunk::text("hello");
        assert!(matches!(text, OutputChunk::Text(_)));

        let error = OutputChunk::error("something wrong");
        assert!(matches!(error, OutputChunk::Error(_)));
    }
}