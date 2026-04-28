//! Slash 命令解析和分发
//!
//! REPL 中以 '/' 开头的命令不会被发送到 LLM，而是由本模块直接处理。

use hermes_checkpoint::CheckpointManager;
use hermes_core::config::ProfileManager;
use std::path::PathBuf;
use std::sync::Arc;

/// 解析后的 slash 命令
#[derive(Debug, Clone)]
pub enum SlashCommand {
    /// /background <prompt> — 后台异步任务
    Background(String),
    /// /yolo — 切换 YOLO 模式
    Yolo,
    /// /fast [on|off] — 设置 Fast 模式
    Fast(Option<bool>),
    /// /rollback [N] [file] — 回滚检查点
    Rollback(Option<usize>, Option<String>),
    /// /profile <name> — 切换配置
    Profile(String),
    /// /profiles — 列出所有配置
    Profiles,
    /// /undo — 撤销上一轮对话
    Undo,
    /// /retry — 重试上一轮请求
    Retry,
    /// /compress — 手动压缩上下文
    Compress,
    /// /usage — 显示 Token 使用统计
    Usage,
    /// /insights — 显示会话洞察
    Insights,
    /// /model [name] — 查看/切换模型
    Model(Option<String>),
    /// /reset — 重置会话
    Reset,
    /// /help — 显示帮助
    Help,
    /// /clear — 清屏
    Clear,
    /// /exit 或 /quit — 退出 REPL
    Exit,
}

/// slash 命令执行结果
pub struct SlashCommandResult {
    /// 显示给用户的消息
    pub message: String,
    /// 是否退出 REPL
    pub should_exit: bool,
}

/// REPL 共享状态
pub struct ReplState {
    /// 当前 YOLO 模式
    pub yolo_mode: bool,
    /// 当前 Fast 模式
    pub fast_mode: bool,
    /// 当前会话 ID
    pub session_id: String,
    /// 当前模型
    pub model: String,
    /// 检查点管理器
    pub checkpoint_manager: Option<Arc<CheckpointManager>>,
    /// 配置文件路径（用于持久化 config 更改）
    pub config_path: PathBuf,
    /// Profile 管理器
    pub profile_manager: Option<Arc<parking_lot::Mutex<ProfileManager>>>,
    /// Token 使用统计 (prompt_tokens, completion_tokens, total_tokens)
    pub token_usage: TokenUsage,
    /// 上一条用户输入（用于 /retry）
    pub last_user_input: Option<String>,
}

/// Token 使用统计
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub request_count: u64,
}

impl ReplState {
    /// 持久化当前 safety 配置到配置文件
    pub fn save_safety_config(&self) -> Result<(), String> {
        let mut config = hermes_core::config::Config::load()
            .map_err(|e| format!("加载配置失败: {}", e))?;

        config.safety.yolo_mode = self.yolo_mode;
        config.safety.fast_mode = self.fast_mode;

        config.save()
            .map_err(|e| format!("保存配置失败: {}", e))?;

        Ok(())
    }
}

/// 解析 slash 命令
///
/// 如果输入不以 '/' 开头，返回 None（表示不是 slash 命令）。
pub fn parse_slash_command(input: &str) -> Option<SlashCommand> {
    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }

    let rest = &input[1..]; // 去掉前缀 '/'
    let parts: Vec<&str> = rest.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).unwrap_or(&"").trim();

    match cmd.as_str() {
        "background" | "bg" => {
            if args.is_empty() {
                Some(SlashCommand::Background(String::new()))
            } else {
                Some(SlashCommand::Background(args.to_string()))
            }
        }
        "yolo" => Some(SlashCommand::Yolo),
        "fast" => {
            let sub = parts.get(1).unwrap_or(&"").trim().to_lowercase();
            match sub.as_str() {
                "on" | "true" | "1" => Some(SlashCommand::Fast(Some(true))),
                "off" | "false" | "0" => Some(SlashCommand::Fast(Some(false))),
                _ => Some(SlashCommand::Fast(None)),
            }
        }
        "rollback" => {
            let args = parts.get(1).unwrap_or(&"").trim();
            if args.is_empty() {
                return Some(SlashCommand::Rollback(None, None));
            }
            let rollback_parts: Vec<&str> = args.splitn(2, ' ').collect();
            let index: Option<usize> = rollback_parts[0].parse().ok();
            let file = rollback_parts.get(1).map(|s| s.to_string());
            Some(SlashCommand::Rollback(index, file))
        }
        "profile" => {
            if args.is_empty() {
                None // 需要参数
            } else {
                Some(SlashCommand::Profile(args.to_string()))
            }
        }
        "profiles" => Some(SlashCommand::Profiles),
        "undo" => Some(SlashCommand::Undo),
        "retry" => Some(SlashCommand::Retry),
        "compress" => Some(SlashCommand::Compress),
        "usage" => Some(SlashCommand::Usage),
        "insights" => Some(SlashCommand::Insights),
        "model" => {
            if args.is_empty() {
                Some(SlashCommand::Model(None))
            } else {
                Some(SlashCommand::Model(Some(args.to_string())))
            }
        }
        "reset" | "new" => Some(SlashCommand::Reset),
        "help" | "h" | "?" => Some(SlashCommand::Help),
        "clear" | "cls" => Some(SlashCommand::Clear),
        "exit" | "quit" | "q" => Some(SlashCommand::Exit),
        _ => None, // 未知的 slash 命令，让它通过给 LLM 处理
    }
}

/// 分发 slash 命令
///
/// 这是异步函数，因为 rollback 需要 git I/O。
pub async fn dispatch_slash_command(
    cmd: SlashCommand,
    state: &mut ReplState,
) -> SlashCommandResult {
    match cmd {
        SlashCommand::Background(prompt) => {
            if prompt.is_empty() {
                return SlashCommandResult {
                    message: "用法: /background <提示词> — 在后台异步执行任务".into(),
                    should_exit: false,
                };
            }
            // 后台任务通过 BackgroundTaskManager 处理，这里只返回提示
            SlashCommandResult {
                message: format!("后台任务需要 BackgroundTaskManager 支持。提示词: {}", prompt),
                should_exit: false,
            }
        }
        SlashCommand::Yolo => {
            state.yolo_mode = !state.yolo_mode;
            let status = if state.yolo_mode { "开启" } else { "关闭" };
            let warning = if let Err(e) = state.save_safety_config() {
                format!(" (配置保存失败: {})", e)
            } else {
                String::new()
            };
            SlashCommandResult {
                message: format!("YOLO 模式: {} — 危险命令审批检查已{}{}", status, if state.yolo_mode { "跳过" } else { "恢复" }, warning),
                should_exit: false,
            }
        }
        SlashCommand::Fast(action) => {
            let new_state = match action {
                Some(true) => true,
                Some(false) => false,
                None => !state.fast_mode, // toggle
            };
            state.fast_mode = new_state;
            let status = if state.fast_mode { "开启" } else { "关闭" };
            let warning = if let Err(e) = state.save_safety_config() {
                format!(" (配置保存失败: {})", e)
            } else {
                String::new()
            };
            SlashCommandResult {
                message: format!("Fast 模式: {}{}", status, warning),
                should_exit: false,
            }
        }
        SlashCommand::Rollback(index, file) => {
            if let Some(cm) = &state.checkpoint_manager {
                let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                match index {
                    None => {
                        // 列出检查点
                        match cm.list_checkpoints(&working_dir, None).await {
                            Ok(entries) => {
                                if entries.is_empty() {
                                    return SlashCommandResult {
                                        message: "没有检查点记录。当工具修改文件时会自动创建检查点。".into(),
                                        should_exit: false,
                                    };
                                }
                                let mut lines = vec!["检查点列表:".to_string()];
                                for (i, entry) in entries.iter().enumerate() {
                                    lines.push(format!(
                                        "  #{}  {}  {}",
                                        i + 1,
                                        entry.commit_hash,
                                        entry.message,
                                    ));
                                }
                                SlashCommandResult {
                                    message: lines.join("\n"),
                                    should_exit: false,
                                }
                            }
                            Err(e) => SlashCommandResult {
                                message: format!("列出检查点失败: {}", e),
                                should_exit: false,
                            },
                        }
                    }
                    Some(n) => {
                        // 先列出以获取 commit hash
                        match cm.list_checkpoints(&working_dir, None).await {
                            Ok(entries) => {
                                if n == 0 || n > entries.len() {
                                    return SlashCommandResult {
                                        message: format!(
                                            "无效的检查点编号: {}。可用范围: 1-{}",
                                            n,
                                            entries.len()
                                        ),
                                        should_exit: false,
                                    };
                                }
                                let entry = &entries[n - 1];
                                let file_path = file.as_ref().map(|f| PathBuf::from(f));

                                // 回滚前自动创建快照
                                let _ = cm.snapshot_working_dir(&working_dir).await;

                                match cm
                                    .restore_checkpoint(
                                        &working_dir,
                                        &entry.commit_hash,
                                        file_path.as_deref(),
                                    )
                                    .await
                                {
                                    Ok(()) => {
                                        let target = if let Some(fp) = &file {
                                            format!(" 文件: {}", std::path::Path::new(fp).display())
                                        } else {
                                            String::new()
                                        };
                                        SlashCommandResult {
                                            message: format!(
                                                "已回滚到检查点 #{}{} ({})",
                                                n, target, entry.message
                                            ),
                                            should_exit: false,
                                        }
                                    }
                                    Err(e) => SlashCommandResult {
                                        message: format!("回滚失败: {}", e),
                                        should_exit: false,
                                    },
                                }
                            }
                            Err(e) => SlashCommandResult {
                                message: format!("列出检查点失败: {}", e),
                                should_exit: false,
                            },
                        }
                    }
                }
            } else {
                SlashCommandResult {
                    message: "检查点系统不可用".into(),
                    should_exit: false,
                }
            }
        }
        SlashCommand::Help => {
            let help_text = r#"
可用命令:
  /background <提示词>  在后台异步执行 AI Agent 任务
  /yolo                 切换 YOLO 模式（跳过危险命令审批）
  /fast [on|off]        切换 Fast 模式（优先级 API 处理）
  /rollback             列出所有文件检查点
  /rollback <N>         回滚到第 N 个检查点
  /rollback <N> <file>  回滚指定文件到第 N 个检查点
  /profile <name>       切换到指定配置
  /profiles             列出所有配置
  /model [name]         查看当前模型或切换到新模型
  /undo                 撤销上一轮对话
  /retry                重试上一轮请求
  /compress             手动压缩上下文
  /usage                显示 Token 使用统计
  /insights             显示会话洞察
  /reset                重置当前会话
  /help                 显示此帮助信息
  /clear                清屏
  /exit                 退出 Hermes Agent

提示: 所有不以 '/' 开头的输入都会被发送给 AI Agent 处理。"#;
            SlashCommandResult {
                message: help_text.to_string(),
                should_exit: false,
            }
        }
        SlashCommand::Profile(name) => {
            if let Some(pm) = &state.profile_manager {
                let mut manager = pm.lock();
                match manager.switch_profile(&name) {
                    Ok(()) => {
                        let config = manager.get_active_config();
                        state.model = config.defaults.model.clone();
                        state.yolo_mode = config.safety.yolo_mode;
                        state.fast_mode = config.safety.fast_mode;
                        SlashCommandResult {
                            message: format!("已切换到配置: {} (模型: {})", name, state.model),
                            should_exit: false,
                        }
                    }
                    Err(e) => SlashCommandResult {
                        message: format!("切换配置失败: {}", e),
                        should_exit: false,
                    },
                }
            } else {
                SlashCommandResult {
                    message: "Profile 系统不可用".into(),
                    should_exit: false,
                }
            }
        }
        SlashCommand::Profiles => {
            if let Some(pm) = &state.profile_manager {
                let manager = pm.lock();
                let profiles = manager.list_profiles();
                let active = manager.get_active_profile_name();

                if profiles.is_empty() {
                    SlashCommandResult {
                        message: "没有可用的配置。使用 /profile create <name> 创建新配置。".into(),
                        should_exit: false,
                    }
                } else {
                    let mut lines = vec!["可用配置:".to_string()];
                    for name in profiles {
                        let marker = if Some(name) == active { " *" } else { "" };
                        lines.push(format!("  - {}{}", name, marker));
                    }
                    lines.push("\n使用 /profile <name> 切换配置".to_string());
                    SlashCommandResult {
                        message: lines.join("\n"),
                        should_exit: false,
                    }
                }
            } else {
                SlashCommandResult {
                    message: "Profile 系统不可用".into(),
                    should_exit: false,
                }
            }
        }
        SlashCommand::Undo => {
            // 撤销功能需要 SessionStore 支持
            SlashCommandResult {
                message: "撤销功能需要会话存储支持。将移除上一轮对话记录。".into(),
                should_exit: false,
            }
        }
        SlashCommand::Retry => {
            if let Some(input) = &state.last_user_input {
                SlashCommandResult {
                    message: format!("将重新执行: {}", input),
                    should_exit: false,
                }
            } else {
                SlashCommandResult {
                    message: "没有可重试的上一轮对话".into(),
                    should_exit: false,
                }
            }
        }
        SlashCommand::Compress => {
            // 压缩功能需要 ContextCompressor 支持
            SlashCommandResult {
                message: "上下文压缩功能即将推出。".into(),
                should_exit: false,
            }
        }
        SlashCommand::Usage => {
            let usage = &state.token_usage;
            let avg_tokens = if usage.request_count > 0 {
                usage.total_tokens / usage.request_count
            } else {
                0
            };
            let message = format!(
                "Token 使用统计:\n  请求次数: {}\n  输入 Tokens: {}\n  输出 Tokens: {}\n  总 Tokens: {}\n  平均 Tokens/请求: {}",
                usage.request_count,
                usage.prompt_tokens,
                usage.completion_tokens,
                usage.total_tokens,
                avg_tokens
            );
            SlashCommandResult {
                message,
                should_exit: false,
            }
        }
        SlashCommand::Insights => {
            // 洞察功能需要 InsightsTracker 支持
            SlashCommandResult {
                message: "会话洞察功能即将推出。".into(),
                should_exit: false,
            }
        }
        SlashCommand::Model(name) => {
            match name {
                Some(new_model) => {
                    state.model = new_model.clone();
                    SlashCommandResult {
                        message: format!("模型已切换到: {}", new_model),
                        should_exit: false,
                    }
                }
                None => {
                    SlashCommandResult {
                        message: format!("当前模型: {}", state.model),
                        should_exit: false,
                    }
                }
            }
        }
        SlashCommand::Reset => {
            SlashCommandResult {
                message: "会话已重置。开始新的对话。".into(),
                should_exit: false,
            }
        }
        SlashCommand::Clear => {
            // 清屏 ANSI 序列
            print!("\x1B[2J\x1B[1;1H");
            SlashCommandResult {
                message: String::new(),
                should_exit: false,
            }
        }
        SlashCommand::Exit => SlashCommandResult {
            message: "再见！".into(),
            should_exit: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yolo() {
        assert!(matches!(parse_slash_command("/yolo"), Some(SlashCommand::Yolo)));
    }

    #[test]
    fn test_parse_fast_toggle() {
        assert!(matches!(parse_slash_command("/fast"), Some(SlashCommand::Fast(None))));
    }

    #[test]
    fn test_parse_fast_on() {
        assert!(matches!(parse_slash_command("/fast on"), Some(SlashCommand::Fast(Some(true)))));
    }

    #[test]
    fn test_parse_fast_off() {
        assert!(matches!(parse_slash_command("/fast off"), Some(SlashCommand::Fast(Some(false)))));
    }

    #[test]
    fn test_parse_background() {
        match parse_slash_command("/background fix the bug") {
            Some(SlashCommand::Background(prompt)) => assert_eq!(prompt, "fix the bug"),
            _ => panic!("expected Background"),
        }
    }

    #[test]
    fn test_parse_rollback() {
        match parse_slash_command("/rollback") {
            Some(SlashCommand::Rollback(None, None)) => {}
            _ => panic!("expected Rollback(None, None)"),
        }
    }

    #[test]
    fn test_parse_rollback_with_index() {
        match parse_slash_command("/rollback 3") {
            Some(SlashCommand::Rollback(Some(3), None)) => {}
            _ => panic!("expected Rollback(Some(3), None)"),
        }
    }

    #[test]
    fn test_parse_rollback_with_file() {
        match parse_slash_command("/rollback 2 src/main.rs") {
            Some(SlashCommand::Rollback(Some(2), Some(file))) => assert_eq!(file, "src/main.rs"),
            _ => panic!("expected Rollback with file"),
        }
    }

    #[test]
    fn test_parse_help() {
        assert!(matches!(parse_slash_command("/help"), Some(SlashCommand::Help)));
    }

    #[test]
    fn test_parse_exit() {
        assert!(matches!(parse_slash_command("/exit"), Some(SlashCommand::Exit)));
        assert!(matches!(parse_slash_command("/quit"), Some(SlashCommand::Exit)));
        assert!(matches!(parse_slash_command("/q"), Some(SlashCommand::Exit)));
    }

    #[test]
    fn test_non_slash_input_returns_none() {
        assert!(parse_slash_command("hello").is_none());
        assert!(parse_slash_command("").is_none());
        assert!(parse_slash_command("  hi").is_none());
    }

    #[test]
    fn test_unknown_slash_returns_none() {
        // 未知 slash 命令返回 None，让 LLM 处理
        assert!(parse_slash_command("/unknowncommand").is_none());
    }
}
