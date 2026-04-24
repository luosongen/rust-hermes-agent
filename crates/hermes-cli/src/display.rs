//! CLI Display — ANSI spinner、工具进度、diff 格式化

use hermes_core::DisplayHandler;
use serde_json::Value;
use std::io::Write;

/// CLI 显示实现
///
/// 使用 ANSI 转义码提供 spinner、工具进度和颜色输出。
pub struct CliDisplay;

impl CliDisplay {
    pub fn new() -> Self {
        Self
    }

    fn spinner_frame() -> &'static str {
        static FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as usize / 80)
            % FRAMES.len();
        FRAMES[idx]
    }
}

impl DisplayHandler for CliDisplay {
    fn tool_started(&self, tool_name: &str, _args: &Value) {
        eprint!("\r{} {} ... ", Self::spinner_frame(), tool_name);
    }

    fn tool_completed(&self, tool_name: &str, _result: &str) {
        eprintln!("\r  {} {} done", green_check(), tool_name);
    }

    fn tool_failed(&self, tool_name: &str, error: &str) {
        eprintln!("\r  {} {} failed: {}", red_cross(), tool_name, error);
    }

    fn thinking_chunk(&self, chunk: &str) {
        eprint!("{}", chunk);
    }

    fn show_diff(&self, filename: &str, old: &str, new: &str) {
        eprintln!("\n  {} {}", yellow_delta(), filename);
        // 简单行级别 diff
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let mut oi = 0;
        let mut ni = 0;
        while oi < old_lines.len() || ni < new_lines.len() {
            if oi < old_lines.len() && ni < new_lines.len() && old_lines[oi] == new_lines[ni] {
                eprintln!("    {}", old_lines[oi]);
                oi += 1;
                ni += 1;
            } else if oi < old_lines.len() {
                eprintln!("  \x1b[31m-   {}\x1b[0m", old_lines[oi]);
                oi += 1;
            } else if ni < new_lines.len() {
                eprintln!("  \x1b[32m+   {}\x1b[0m", new_lines[ni]);
                ni += 1;
            }
        }
    }

    fn spinner_start(&self, message: &str) {
        eprint!("\r{} {}", Self::spinner_frame(), message);
    }

    fn spinner_stop(&self) {
        eprint!("\r\x1b[K");
    }

    fn flush(&self) {
        let _ = std::io::stderr().flush();
    }

    fn show_usage(&self, insights: &hermes_core::SessionInsights) {
        let input_k = insights.input_tokens as f64 / 1000.0;
        let output_k = insights.output_tokens as f64 / 1000.0;
        let cost = insights.estimated_cost_usd;

        // Count tool calls by name
        let mut tool_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for call in &insights.tool_calls {
            *tool_counts.entry(call.tool_name.as_str()).or_insert(0) += 1;
        }

        let tools_str = if tool_counts.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = tool_counts
                .iter()
                .map(|(name, count)| format!("{}({})", name, count))
                .collect();
            format!(" | {}", parts.join(", "))
        };

        eprint!(
            "\r\x1b[K[Tokens: {:.1}K/{:.1}K | Cost: ${:.4}{}]",
            input_k, output_k, cost, tools_str
        );
    }
}

impl Default for CliDisplay {
    fn default() -> Self {
        Self::new()
    }
}

fn green_check() -> &'static str {
    "\x1b[32m✓\x1b[0m"
}

fn red_cross() -> &'static str {
    "\x1b[31m✗\x1b[0m"
}

fn yellow_delta() -> &'static str {
    "\x1b[33mΔ\x1b[0m"
}
