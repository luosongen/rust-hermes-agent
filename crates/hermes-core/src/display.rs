//! Display Handler — Agent 执行工具和思考时的显示反馈接口

use serde_json::Value;

/// 显示处理 trait — Agent 工具执行和思考的显示反馈
///
/// 所有方法均为同步（显示操作通常很快，不需要 async）。
/// CLI 实现使用 ANSI 颜色和 spinner，平台适配器可实现 Webhook 通知。
pub trait DisplayHandler: Send + Sync {
    /// 工具开始执行
    fn tool_started(&self, tool_name: &str, args: &Value);

    /// 工具执行完成
    fn tool_completed(&self, tool_name: &str, result: &str);

    /// 工具执行失败
    fn tool_failed(&self, tool_name: &str, error: &str);

    /// 显示思考/推理内容（流式）
    fn thinking_chunk(&self, chunk: &str);

    /// 显示 diff（文件修改）
    fn show_diff(&self, filename: &str, old: &str, new: &str);

    /// 显示 spinner（开始）
    fn spinner_start(&self, message: &str);

    /// 停止 spinner
    fn spinner_stop(&self);

    /// 刷新显示
    fn flush(&self);
}

/// 默认无操作实现（当没有 display handler 注册时使用）
pub struct NoopDisplay;

impl DisplayHandler for NoopDisplay {
    fn tool_started(&self, _tool_name: &str, _args: &Value) {}
    fn tool_completed(&self, _tool_name: &str, _result: &str) {}
    fn tool_failed(&self, _tool_name: &str, _error: &str) {}
    fn thinking_chunk(&self, _chunk: &str) {}
    fn show_diff(&self, _filename: &str, _old: &str, _new: &str) {}
    fn spinner_start(&self, _message: &str) {}
    fn spinner_stop(&self) {}
    fn flush(&self) {}
}

impl NoopDisplay {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopDisplay {
    fn default() -> Self {
        Self::new()
    }
}
