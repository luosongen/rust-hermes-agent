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
