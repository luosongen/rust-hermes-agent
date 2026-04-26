//! 技能执行模块
//!
//! 提供技能逐步执行的能力，解析技能中的 checkbox 步骤并跟踪执行状态。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;

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
    pub task_idx: usize,
    pub step_idx: usize,
    pub task_name: String,
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
