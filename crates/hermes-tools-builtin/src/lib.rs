//! hermes-tools-builtin — 内置工具集
//!
//! 本 crate 提供了 AI Agent 开箱即用的内置工具实现，包括文件操作、终端执行和技能管理。
//!
//! ## 模块结构
//! - **`file_tools`** — 文件读写工具：`ReadFileTool`、`WriteFileTool`
//! - **`terminal_tools`** — 终端执行工具：`TerminalTool`
//! - **`skills`** — 技能管理工具：`SkillExecuteTool`、`SkillListTool`、`SkillSearchTool`
//!
//! ## 主要类型
//! - **`ReadFileTool`** — 按路径读取文件内容，支持偏移量和行数限制
//! - **`WriteFileTool`** — 创建或覆盖写入文件
//! - **`TerminalTool`** — 在工作目录下执行 shell 命令
//! - **`SkillExecuteTool`** — 根据名称执行已注册的 Hermes 技能
//! - **`SkillListTool`** — 列出所有可用的技能名称
//! - **`SkillSearchTool`** — 按名称或描述搜索技能
//!
//! ## 与其他模块的关系
//! - 依赖 `hermes-tool-registry` 中的 `Tool` trait 和 `ToolRegistry`
//! - 依赖 `hermes-core` 中的 `ToolContext`、`ToolError` 等类型
//! - 技能模块依赖外部 `hermes_skills` crate 提供技能加载和注册
//! - 通过 `register_builtin_tools()` 函数将所有内置工具注册到传入的 `ToolRegistry`
//!
//! ## 安全说明
//! - `ReadFileTool` 使用 `canonicalize()` 防止路径遍历攻击
//! - `TerminalTool` 仅支持简单命令拆分，不支持 shell 管道和复杂语法
//! - 所有工具都基于 `ToolContext` 中的 `working_directory` 做相对路径解析

/// 文件读写工具模块
pub mod file_tools;
/// 技能管理工具模块
pub mod skills;
/// 终端执行工具模块
pub mod terminal_tools;
/// 任务列表管理工具模块
pub mod todo_tools;
/// 用户交互工具模块
pub mod clarify_tools;

pub use file_tools::{ReadFileTool, WriteFileTool};
pub use skills::{load_skill_registry, SkillExecuteTool, SkillListTool, SkillSearchTool};
pub use terminal_tools::TerminalTool;
pub use todo_tools::{TodoStore, TodoTool};
pub use clarify_tools::ClarifyTool;

use hermes_tool_registry::ToolRegistry;

/// 将所有内置工具注册到传入的 ToolRegistry
///
/// 注册的工具包括：
/// - `ReadFileTool` - 文件读取工具
/// - `WriteFileTool` - 文件写入工具
/// - `TerminalTool` - 终端执行工具
///
/// 注意：技能相关工具需要单独创建并注册（依赖 SkillRegistry）
pub fn register_builtin_tools(registry: &ToolRegistry) {
    registry.register(ReadFileTool::new());
    registry.register(WriteFileTool::new());
    registry.register(TerminalTool::new());
    registry.register(TodoTool::new());
}
