pub mod file_tools;
pub mod skills;
pub mod terminal_tools;

pub use file_tools::{ReadFileTool, WriteFileTool};
pub use skills::{load_skill_registry, SkillExecuteTool, SkillListTool, SkillSearchTool};
pub use terminal_tools::TerminalTool;

use hermes_tool_registry::ToolRegistry;

pub fn register_builtin_tools(registry: &ToolRegistry) {
    registry.register(ReadFileTool::new());
    registry.register(WriteFileTool::new());
    registry.register(TerminalTool::new());
}
