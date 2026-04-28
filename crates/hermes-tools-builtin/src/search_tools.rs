//! search_tools — 文件搜索工具（ripgrep + grep 降级）

use async_trait::async_trait;
use hermes_core::{ToolContext, ToolError};
use hermes_environment::Environment;
use serde_json::json;
use std::sync::Arc;

/// 文件搜索工具
///
/// 使用 ripgrep（rg）进行高速文件内容搜索，rg 不可用时降级到 grep。
pub struct SearchFilesTool {
    environment: Arc<dyn Environment>,
}

impl SearchFilesTool {
    pub fn new(environment: Arc<dyn Environment>) -> Self {
        Self { environment }
    }

    /// 检测 rg 是否可用
    async fn has_rg(&self) -> bool {
        self.environment
            .which("rg")
            .await
            .unwrap_or(None)
            .is_some()
    }

    /// 构建 rg 命令参数
    fn build_rg_args(
        pattern: &str,
        path: &str,
        include: Option<&str>,
        file_types: Option<&str>,
        output_mode: &str,
        context_lines: i64,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "--line-number".into(),
            "--no-heading".into(),
            "--with-filename".into(),
        ];

        if let Some(g) = include {
            args.push("--glob".into());
            args.push(g.to_string());
        }
        if let Some(t) = file_types {
            args.push("--type".into());
            args.push(t.to_string());
        }
        if context_lines > 0 {
            args.push("-C".into());
            args.push(context_lines.to_string());
        }
        match output_mode {
            "files_with_matches" => {
                args.push("-l".into());
            }
            "count" => {
                args.push("-c".into());
            }
            _ => {} // content mode, default
        }

        args.push(pattern.to_string());
        args.push(path.to_string());
        args
    }

    /// 构建 grep 命令参数（降级方案）
    fn build_grep_args(
        pattern: &str,
        path: &str,
        include: Option<&str>,
        output_mode: &str,
        context_lines: i64,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec!["-rnH".into()];
        args.push("--exclude-dir=.*".into());

        if let Some(g) = include {
            args.push("--include".into());
            args.push(g.to_string());
        }
        if context_lines > 0 {
            args.push(format!("-C{}", context_lines));
        }
        match output_mode {
            "files_with_matches" => {
                args.push("-l".into());
            }
            "count" => {
                args.push("-c".into());
            }
            _ => {}
        }

        args.push(pattern.to_string());
        args.push(path.to_string());
        args
    }

    /// 解析 rg/grep 输出为结构化匹配结果
    fn parse_output(
        stdout: &str,
        output_mode: &str,
        context_lines: i64,
    ) -> Vec<serde_json::Value> {
        let mut matches = Vec::new();

        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // 跳过分隔符 --
            if line == "--" {
                continue;
            }

            match output_mode {
                "files_with_matches" => {
                    matches.push(json!({"file": line}));
                }
                "count" => {
                    // 格式: file:count
                    if let Some((file, count)) = line.rsplit_once(':') {
                        matches.push(json!({
                            "file": file,
                            "count": count.trim().parse::<i64>().unwrap_or(0),
                        }));
                    }
                }
                _ => {
                    // content 模式: file:line:text 或 file-line-text（context 行）
                    if let Some(rest) = line.find(':') {
                        let mut parts: Vec<&str> = line.splitn(3, ':').collect();
                        if parts.len() >= 2 {
                            let file = parts[0].to_string();
                            let line_num: Option<u32> = parts[1].parse().ok();
                            let text = if parts.len() > 2 {
                                parts[2].to_string()
                            } else {
                                String::new()
                            };

                            let is_context = line_num.is_some()
                                && context_lines > 0
                                && line.contains('-');

                            matches.push(json!({
                                "file": file,
                                "line": line_num,
                                "content": text,
                                "is_context": is_context,
                            }));
                        }
                    }
                }
            }
        }

        matches
    }
}

#[async_trait]
impl hermes_tool_registry::Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }

    fn description(&self) -> &str {
        "使用 ripgrep 进行高速文件内容搜索。支持正则表达式、文件过滤、输出模式（content/files_with_matches/count）和上下文行数。"
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "搜索的正则表达式模式"
                },
                "path": {
                    "type": "string",
                    "description": "搜索目录或文件路径",
                    "default": "."
                },
                "include": {
                    "type": "string",
                    "description": "文件 glob 过滤（如 \"*.rs\"）"
                },
                "file_types": {
                    "type": "string",
                    "description": "ripgrep 文件类型（如 \"rust\", \"py\"）"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "输出模式: content=显示匹配行, files_with_matches=只显示文件名, count=显示每文件匹配数",
                    "default": "content"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "每个匹配前后显示的上下文行数",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "最大返回结果数",
                    "default": 50
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        context: ToolContext,
    ) -> Result<String, ToolError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs("pattern 为必填参数".into()))?;

        if pattern.is_empty() {
            return Err(ToolError::InvalidArgs("pattern 不能为空".into()));
        }

        let path = args["path"].as_str().unwrap_or(".");
        let include = args["include"].as_str();
        let file_types = args["file_types"].as_str();
        let output_mode = args["output_mode"].as_str().unwrap_or("content");
        let context_lines = args["context_lines"].as_i64().unwrap_or(0);
        let limit = args["limit"].as_u64().unwrap_or(50) as usize;

        // 确保路径相对于工作目录
        let search_path = if path.starts_with('/') {
            path.to_string()
        } else {
            context
                .working_directory
                .join(path)
                .to_string_lossy()
                .to_string()
        };

        let max_results: usize = 500;
        let fetch_limit = max_results;

        let (program, arguments) = if self.has_rg().await {
            let args = Self::build_rg_args(pattern, &search_path, include, file_types, output_mode, context_lines);
            ("rg", args)
        } else {
            let args = Self::build_grep_args(pattern, &search_path, include, output_mode, context_lines);
            ("grep", args)
        };

        // 转换参数为 &str slice
        let args_str: Vec<&str> = arguments.iter().map(|s| s.as_str()).collect();

        let result = self
            .environment
            .execute(program, &args_str, None, Some(std::time::Duration::from_secs(30)), None)
            .await
            .map_err(|e| ToolError::Execution(format!("搜索执行失败: {}", e)))?;

        let truncated = result.stdout.lines().count() > fetch_limit;
        let mut matches = Self::parse_output(&result.stdout, output_mode, context_lines);
        let total_count = matches.len();

        if matches.len() > limit {
            matches.truncate(limit);
        }

        Ok(json!({
            "success": result.success || !matches.is_empty(),
            "command": format!("{} {}", program, arguments.join(" ")),
            "matches": matches,
            "count": total_count,
            "truncated": truncated || total_count > limit,
            "exit_code": result.exit_code,
        })
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_content_output() {
        let stdout = "src/main.rs:10:fn main() {\nsrc/lib.rs:5:pub mod test;\n";
        let results = SearchFilesTool::parse_output(stdout, "content", 0);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["file"], "src/main.rs");
        assert_eq!(results[0]["line"], 10);
    }

    #[test]
    fn test_parse_files_only_output() {
        let stdout = "src/main.rs\nsrc/lib.rs\n";
        let results = SearchFilesTool::parse_output(stdout, "files_with_matches", 0);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["file"], "src/main.rs");
    }

    #[test]
    fn test_parse_count_output() {
        let stdout = "src/main.rs:5\nsrc/lib.rs:3\n";
        let results = SearchFilesTool::parse_output(stdout, "count", 0);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["count"], 5);
    }

    #[test]
    fn test_parse_empty_output() {
        let results = SearchFilesTool::parse_output("", "content", 0);
        assert_eq!(results.len(), 0);
    }
}
