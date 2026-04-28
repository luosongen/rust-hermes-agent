//! hermes-tools-builtin — 内置工具集
//!
//! 本 crate 提供了 AI Agent 开箱即用的内置工具实现，包括文件操作、终端执行和技能管理。
//!
//! ## 模块结构
//! - **`file_tools`** — 文件读写工具：`ReadFileTool`、`WriteFileTool`
//! - **`terminal_tools`** — 终端执行工具：`TerminalTool`
//! - **`skills`** — 技能管理工具：`SkillExecuteTool`、`SkillListTool`、`SkillSearchTool`
//! - **`browser_tools`** — 浏览器自动化工具：`BrowserNavigateTool`、`BrowserSnapshotTool` 等
//!
//! ## 主要类型
//! - **`ReadFileTool`** — 按路径读取文件内容，支持偏移量和行数限制
//! - **`WriteFileTool`** — 创建或覆盖写入文件
//! - **`TerminalTool`** — 在工作目录下执行 shell 命令
//! - **`SkillExecuteTool`** — 根据名称执行已注册的 Hermes 技能
//! - **`SkillListTool`** — 列出所有可用的技能名称
//! - **`SkillSearchTool`** — 按名称或描述搜索技能
//! - **`BrowserNavigateTool`** — 浏览器导航（通过 agent-browser CLI）
//! - **`BrowserSnapshotTool`** — 获取页面可访问性快照
//! - **`BrowserClickTool`** — 点击页面元素
//! - **`BrowserTypeTool`** — 向输入框填写文本
//! - **`BrowserScrollTool`** — 滚动页面
//! - **`BrowserBackTool`** — 浏览器后退
//! - **`BrowserPressTool`** — 键盘按键
//! - **`BrowserVisionTool`** — 页面截图（Vision AI 分析待集成）
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
/// 危险命令审批工具模块
pub mod approval_tools;
/// 浏览器自动化工具模块
pub mod browser_tools;
/// 文件搜索工具模块
pub mod search_tools;
/// 文件补丁工具模块
pub mod patch_tools;
/// 网络搜索工具模块
pub mod web_tools;

pub use file_tools::{ReadFileTool, WriteFileTool};
pub use skills::{load_skill_registry, load_skill_registry_and_manager, SkillExecuteTool, SkillListTool, SkillSearchTool, SkillManageTool};
pub use terminal_tools::TerminalTool;
pub use todo_tools::{TodoStore, TodoTool};
pub use clarify_tools::ClarifyTool;
pub use approval_tools::{ApprovalStore, ApprovalTool};
pub use browser_tools::{
    BrowserSessionStore, BrowserToolCore,
    BrowserNavigateTool, BrowserSnapshotTool, BrowserClickTool,
    BrowserTypeTool, BrowserScrollTool, BrowserBackTool, BrowserPressTool,
    BrowserVisionTool,
};
pub use search_tools::SearchFilesTool;
pub use patch_tools::PatchTool;
pub use web_tools::{WebSearchTool, WebSearchConfig, SearchProvider};

use std::path::PathBuf;
use std::sync::Arc;

use hermes_environment::Environment;
use hermes_skills::manager::SkillManager;
use hermes_skills::{SkillExecutor, SkillRegistry};
use hermes_tool_registry::ToolRegistry;
use parking_lot::RwLock;

/// 将所有内置工具注册到传入的 ToolRegistry
///
/// 注册的工具包括：
/// - `ReadFileTool` - 文件读取工具（通过 Environment 后端）
/// - `WriteFileTool` - 文件写入工具（通过 Environment 后端）
/// - `TerminalTool` - 终端执行工具（通过 Environment 后端）
/// - `TodoTool` - 任务列表管理工具
/// - `ApprovalTool` - 危险命令审批工具
/// - `Browser*Tool` - 浏览器自动化工具
///
/// # 参数
/// - `registry` — 工具注册表
/// - `environment` — 执行环境后端（本地、Docker、SSH 等）
///
/// 注意：技能相关工具需要单独创建并注册（依赖 SkillRegistry）
pub fn register_builtin_tools(registry: &ToolRegistry, environment: Arc<dyn Environment>) {
    registry.register(ReadFileTool::new(environment.clone()));
    registry.register(WriteFileTool::new(environment.clone()));
    registry.register(TerminalTool::new(environment.clone()));
    registry.register(TodoTool::new());
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config/hermes-agent"));
    registry.register(ApprovalTool::new(config_dir.clone()));
    // Browser tools
    let browser_core = browser_tools::BrowserToolCore::new(config_dir.clone());
    registry.register(BrowserNavigateTool::new(browser_core.clone()));
    registry.register(BrowserSnapshotTool::new(browser_core.clone()));
    registry.register(BrowserClickTool::new(browser_core.clone()));
    registry.register(BrowserTypeTool::new(browser_core.clone()));
    registry.register(BrowserScrollTool::new(browser_core.clone()));
    registry.register(BrowserBackTool::new(browser_core.clone()));
    registry.register(BrowserPressTool::new(browser_core.clone()));
    registry.register(BrowserVisionTool::new(browser_core.clone()));
    browser_core.start_cleanup();
    // 文件搜索和补丁工具
    registry.register(SearchFilesTool::new(environment.clone()));
    registry.register(PatchTool::new(environment.clone()));
}

/// 注册技能管理工具
pub fn register_skill_tools(registry: &ToolRegistry, manager: Arc<RwLock<SkillManager>>, executor: Arc<SkillExecutor>) {
    registry.register(SkillManageTool::new(manager));
    registry.register(SkillExecuteTool::new(Arc::new(RwLock::new(SkillRegistry::new())), executor));
}
