//! Completer — 斜杠命令自动补全

use std::collections::HashMap;

/// 命令元数据
#[derive(Debug, Clone)]
pub struct CommandMetadata {
    pub name: String,
    pub description: String,
    pub usage: Option<String>,
    pub example: Option<String>,
}

impl CommandMetadata {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            usage: None,
            example: None,
        }
    }

    pub fn with_usage(mut self, usage: &str) -> Self {
        self.usage = Some(usage.to_string());
        self
    }

    pub fn with_example(mut self, example: &str) -> Self {
        self.example = Some(example.to_string());
        self
    }
}

/// 斜杠命令补全器
pub struct SlashCommandCompleter {
    commands: HashMap<String, CommandMetadata>,
}

impl SlashCommandCompleter {
    pub fn new() -> Self {
        let mut completer = Self {
            commands: HashMap::new(),
        };
        completer.register_default_commands();
        completer
    }

    /// 注册默认命令
    fn register_default_commands(&mut self) {
        let defaults = vec![
            CommandMetadata::new("help", "显示帮助信息"),
            CommandMetadata::new("clear", "清除当前屏幕"),
            CommandMetadata::new("exit", "退出 REPL"),
            CommandMetadata::new("quit", "退出 REPL"),
            CommandMetadata::new("mode", "切换对话模式")
                .with_usage("/mode <mode_name>")
                .with_example("/mode coding"),
            CommandMetadata::new("model", "选择模型")
                .with_usage("/model <model_id>")
                .with_example("/model openai/gpt-4o"),
            CommandMetadata::new("context", "显示当前上下文状态"),
            CommandMetadata::new("tokens", "显示当前 token 使用情况"),
            CommandMetadata::new("history", "显示命令历史"),
            CommandMetadata::new("system", "显示系统信息"),
            CommandMetadata::new("retry", "重试上次请求")
                .with_example("/retry"),
            CommandMetadata::new("abort", "中止当前正在进行的请求")
                .with_example("/abort"),
            CommandMetadata::new("compress", "手动触发上下文压缩")
                .with_example("/compress"),
        ];

        for cmd in defaults {
            self.commands.insert(cmd.name.clone(), cmd);
        }
    }

    /// 注册自定义命令
    pub fn register(&mut self, metadata: CommandMetadata) {
        self.commands.insert(metadata.name.clone(), metadata);
    }

    /// 获取所有命令名称
    pub fn command_names(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// 获取命令元数据
    pub fn get(&self, name: &str) -> Option<&CommandMetadata> {
        self.commands.get(name)
    }

    /// 补全命令前缀
    pub fn complete(&self, prefix: &str) -> Vec<String> {
        let prefix_lower = prefix.to_lowercase();
        self.commands
            .keys()
            .filter(|name| name.to_lowercase().starts_with(&prefix_lower))
            .map(|name| format!("/{}", name))
            .collect()
    }

    /// 补全命令参数
    pub fn complete_args(&self, command: &str, _partial: &str) -> Vec<String> {
        match command.trim_start_matches('/').split_whitespace().next().unwrap_or("") {
            "model" => vec![
                "openai/gpt-4o".to_string(),
                "openai/gpt-4o-mini".to_string(),
                "anthropic/claude-3-5-sonnet-20241022".to_string(),
            ],
            "context" => vec!["compress".to_string(), "clear".to_string(), "status".to_string()],
            "tokens" => vec!["status".to_string()],
            "system" => vec!["prompt".to_string(), "role".to_string()],
            _ => vec![],
        }
    }

    /// 获取帮助文本
    pub fn get_help(&self, command: Option<&str>) -> String {
        match command {
            Some(cmd) => {
                if let Some(meta) = self.commands.get(cmd) {
                    let mut help = format!("命令: /{}\n描述: {}\n", meta.name, meta.description);
                    if let Some(usage) = &meta.usage {
                        help.push_str(&format!("用法: {}\n", usage));
                    }
                    if let Some(example) = &meta.example {
                        help.push_str(&format!("示例: {}\n", example));
                    }
                    help
                } else {
                    format!("未知命令: /{}\n输入 /help 查看所有命令", cmd)
                }
            }
            None => {
                let mut help = "可用命令:\n".to_string();
                for (name, meta) in &self.commands {
                    help.push_str(&format!("  /{} - {}\n", name, meta.description));
                }
                help.push_str("\n输入 /help <命令> 查看详细用法");
                help
            }
        }
    }
}

impl Default for SlashCommandCompleter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_commands() {
        let completer = SlashCommandCompleter::new();
        assert!(completer.get("help").is_some());
        assert!(completer.get("clear").is_some());
        assert!(completer.get("exit").is_some());
    }

    #[test]
    fn test_complete_prefix() {
        let completer = SlashCommandCompleter::new();
        let results = completer.complete("/h");
        assert!(results.iter().any(|r| r == "/help"));
    }

    #[test]
    fn test_complete_case_insensitive() {
        let completer = SlashCommandCompleter::new();
        let results = completer.complete("/H");
        assert!(results.iter().any(|r| r == "/help"));
    }

    #[test]
    fn test_register_custom_command() {
        let mut completer = SlashCommandCompleter::new();
        completer.register(
            CommandMetadata::new("custom", "自定义命令")
                .with_usage("/custom <arg>")
                .with_example("/custom test"),
        );
        assert!(completer.get("custom").is_some());
    }

    #[test]
    fn test_get_help() {
        let completer = SlashCommandCompleter::new();
        let help = completer.get_help(None);
        assert!(help.contains("/help"));
    }
}