//! 技能执行模块
//!
//! 提供技能逐步执行的能力，解析技能中的 checkbox 步骤并跟踪执行状态。

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::error::SkillError;
use crate::loader::SkillLoader;
use crate::registry::SkillRegistry;

/// 技能执行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecution {
    /// 技能名称
    pub skill_name: String,
    /// 当前任务索引
    pub current_task: usize,
    /// 当前步骤索引
    pub current_step: usize,
    /// 已完成的步骤列表 (task_idx, step_idx)
    pub completed_steps: Vec<(usize, usize)>,
    /// 所有解析出的步骤
    pub steps: Vec<Step>,
}

impl SkillExecution {
    pub fn new(skill_name: String, steps: Vec<Step>) -> Self {
        Self {
            skill_name,
            current_task: 0,
            current_step: 0,
            completed_steps: Vec::new(),
            steps,
        }
    }

    /// 获取当前步骤
    pub fn current_step(&self) -> Option<&Step> {
        self.steps.iter().find(|s| s.task_idx == self.current_task && s.step_idx == self.current_step)
    }

    /// 是否全部完成
    pub fn is_complete(&self) -> bool {
        self.current_task >= self.steps.len()
            || (self.current_task == self.steps.len() - 1
                && self.current_step >= self.steps.iter().filter(|s| s.task_idx == self.current_task).count())
    }
}

/// 单个步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// 任务索引
    pub task_idx: usize,
    /// 步骤索引
    pub step_idx: usize,
    /// 任务名称
    pub task_name: String,
    /// 步骤内容
    pub content: String,
}

/// 解析技能内容中的步骤
pub fn parse_steps(content: &str) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut current_task_idx = 0;
    let mut current_task_name = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // 检测任务标题 (## Task N: name 或 ## name)
        if trimmed.starts_with("## ") {
            current_task_idx += 1;
            current_task_name = trimmed.trim_start_matches("## ").trim().to_string();
            continue;
        }

        // 检测 checkbox 步骤 (- [ ] 或 - [x])
        if trimmed.starts_with("- [") && trimmed.contains("]") {
            if let Some(content_start) = trimmed.find("] ") {
                let step_content = trimmed[content_start + 2..].trim().to_string();
                if !step_content.is_empty() {
                    let step_idx = steps.iter()
                        .filter(|s| s.task_idx == current_task_idx)
                        .count();
                    steps.push(Step {
                        task_idx: current_task_idx,
                        step_idx,
                        task_name: current_task_name.clone(),
                        content: step_content,
                    });
                }
            }
        }
    }

    steps
}

/// 技能执行器
///
/// 负责跟踪技能执行状态，解析步骤并管理执行进度
pub struct SkillExecutor {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillExecutor {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }

    /// 从默认目录加载技能并创建执行器
    pub fn from_default_dirs() -> Result<Self, SkillError> {
        let loader = SkillLoader::new(SkillLoader::default_dirs());
        let skills = loader.load_all()?;
        let registry = Arc::new(RwLock::new(SkillRegistry::new()));
        for skill in skills {
            registry.write().register(skill)
                .map_err(|e| SkillError::LoadError(e.to_string()))?;
        }
        Ok(Self::new(registry))
    }

    /// 开始执行技能
    pub fn start(&self, skill_name: &str) -> Result<SkillExecution, SkillError> {
        let registry = self.registry.read();
        let skill = registry.get(skill_name)
            .ok_or_else(|| SkillError::NotFound(skill_name.to_string()))?;

        let steps = parse_steps(&skill.content);
        if steps.is_empty() {
            return Err(SkillError::InvalidInput(
                format!("Skill '{}' has no executable steps", skill_name)
            ));
        }

        Ok(SkillExecution::new(skill_name.to_string(), steps))
    }

    /// 获取当前步骤内容
    pub fn get_current_step(&self, execution: &SkillExecution) -> Option<String> {
        execution.current_step().map(|step| {
            format!("## {}\n- [ ] {}\n", step.task_name, step.content)
        })
    }

    /// 标记当前步骤完成并移动到下一步
    pub fn complete_step(&self, execution: &mut SkillExecution) {
        let completed = (execution.current_task, execution.current_step);
        if !execution.completed_steps.contains(&completed) {
            execution.completed_steps.push(completed);
        }

        // 移动到下一步
        let remaining: Vec<_> = execution.steps.iter()
            .filter(|s| !execution.completed_steps.contains(&(s.task_idx, s.step_idx)))
            .collect();

        if let Some(next) = remaining.first() {
            execution.current_task = next.task_idx;
            execution.current_step = next.step_idx;
        } else {
            // 所有步骤完成
            execution.current_task = execution.steps.len();
            execution.current_step = 0;
        }
    }

    /// 获取进度摘要
    pub fn get_progress_summary(&self, execution: &SkillExecution) -> String {
        let total = execution.steps.len();
        let completed = execution.completed_steps.len();
        let remaining = total - completed;

        if remaining == 0 {
            return format!("✅ Skill '{}' completed ({}/{} steps)", execution.skill_name, completed, total);
        }

        let current = execution.current_step()
            .map(|s| format!("Current: {} - {}", s.task_name, s.content))
            .unwrap_or_else(|| "Unknown".to_string());

        format!(
            "📋 {}: {}/{} steps completed, {} remaining\n{}",
            execution.skill_name, completed, total, remaining, current
        )
    }

    /// 获取所有剩余步骤
    pub fn get_remaining_steps(&self, execution: &SkillExecution) -> String {
        let remaining: Vec<_> = execution.steps.iter()
            .filter(|s| !execution.completed_steps.contains(&(s.task_idx, s.step_idx)))
            .collect();

        if remaining.is_empty() {
            return "All steps completed!".to_string();
        }

        let mut output = String::new();
        let mut current_task = 0;
        for step in remaining {
            if step.task_idx != current_task {
                if current_task != 0 {
                    output.push('\n');
                }
                output.push_str(&format!("## {}\n", step.task_name));
                current_task = step.task_idx;
            }
            output.push_str(&format!("- [ ] {}\n", step.content));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_steps() {
        let content = r##"# Test Skill

## Task 1: First Task
- [ ] Step one
- [ ] Step two

## Task 2: Second Task
- [ ] Step three
"##;

        let steps = parse_steps(content);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].task_idx, 1);
        assert_eq!(steps[0].content, "Step one");
        assert_eq!(steps[1].task_idx, 1);
        assert_eq!(steps[1].step_idx, 1);
        assert_eq!(steps[2].task_idx, 2);
    }

    #[test]
    fn test_parse_no_steps() {
        let content = "# No Steps Skill\n\nJust content.";
        let steps = parse_steps(content);
        assert!(steps.is_empty());
    }
}
