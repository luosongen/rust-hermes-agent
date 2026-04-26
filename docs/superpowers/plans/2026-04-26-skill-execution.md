# Skill 主动执行实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现技能主动执行功能，让 Agent 能逐步执行技能工作流，带进度跟踪

**Architecture:** 新增 SkillExecutor 组件解析技能 checkbox 步骤，扩展 skill_execute 工具支持 start/complete/status 操作，状态存储在 SessionStore 中

**Tech Stack:** Rust, parking_lot, serde_json

---

## 文件结构

```
crates/hermes-skills/src/
├── executor.rs         # 新增：SkillExecutor 和 SkillExecution
├── lib.rs              # 修改：导出 SkillExecutor
└── ...

crates/hermes-tools-builtin/src/
└── skills.rs          # 修改：扩展 skill_execute 工具
```

---

## Task 1: 创建 SkillExecution 类型

**Files:**
- Create: `crates/hermes-skills/src/executor.rs`

- [ ] **Step 1: 创建 executor.rs 文件结构**

```rust
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
```

- [ ] **Step 2: 运行 cargo check 验证编译**

Run: `cargo check -p hermes-skills`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-skills/src/executor.rs
git commit -m "feat(skills): 添加 SkillExecution 类型"
```

---

## Task 2: 实现步骤解析

**Files:**
- Modify: `crates/hermes-skills/src/executor.rs`

- [ ] **Step 1: 添加 parse_steps 函数**

```rust
/// 解析技能内容中的步骤
pub fn parse_steps(content: &str) -> Vec<Step> {
    let mut steps = Vec::new();
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
```

- [ ] **Step 2: 添加测试**

```rust
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
```

- [ ] **Step 3: 运行测试验证**

Run: `cargo test -p hermes-skills -- --nocapture`
Expected: 测试通过

- [ ] **Step 4: 提交**

```bash
git add crates/hermes-skills/src/executor.rs
git commit -m "feat(skills): 实现 parse_steps 步骤解析"
```

---

## Task 3: 实现 SkillExecutor

**Files:**
- Modify: `crates/hermes-skills/src/executor.rs`

- [ ] **Step 1: 添加 SkillExecutor 结构体**

```rust
use crate::error::SkillError;
use crate::loader::SkillLoader;
use crate::registry::SkillRegistry;

/// 技能执行器
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
```

- [ ] **Step 2: 提交**

```bash
git add crates/hermes-skills/src/executor.rs
git commit -m "feat(skills): 实现 SkillExecutor 执行器"
```

---

## Task 4: 导出 SkillExecutor

**Files:**
- Modify: `crates/hermes-skills/src/lib.rs`

- [ ] **Step 1: 添加模块和导出**

```rust
pub mod executor;
// ... existing exports ...
pub use executor::{SkillExecutor, SkillExecution, Step};
```

- [ ] **Step 2: 运行 cargo check 验证**

Run: `cargo check -p hermes-skills`
Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add crates/hermes-skills/src/lib.rs
git commit -m "feat(skills): 导出 SkillExecutor"
```

---

## Task 5: 扩展 skill_execute 工具

**Files:**
- Modify: `crates/hermes-tools-builtin/src/skills.rs`

- [ ] **Step 1: 添加 SkillExecuteInput 结构体**

```rust
/// 扩展的 skill_execute 输入参数
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SkillExecuteInput {
    name: String,
    action: String,  // "start" | "continue" | "complete" | "status"
    #[serde(default)]
    skill_content: Option<String>,
}
```

- [ ] **Step 2: 修改 SkillExecuteTool 结构体添加执行器**

```rust
pub struct SkillExecuteTool {
    registry: Arc<RwLock<SkillRegistry>>,
    executor: Arc<SkillExecutor>,
    /// 正在执行的技能状态 (session_id -> execution)
    executions: Arc<RwLock<HashMap<String, SkillExecution>>>,
}
```

- [ ] **Step 3: 修改 new 函数**

```rust
impl SkillExecuteTool {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>, executor: Arc<SkillExecutor>) -> Self {
        Self {
            registry,
            executor,
            executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
```

- [ ] **Step 4: 修改 execute 方法**

```rust
async fn execute(
    &self,
    args: serde_json::Value,
    context: ToolContext,
) -> Result<String, ToolError> {
    let input: SkillExecuteInput = serde_json::from_value(args)
        .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

    // 从 context 获取 session_id
    let session_id = context.session_id
        .ok_or_else(|| ToolError::InvalidArgs("missing session_id".into()))?;

    match input.action.as_str() {
        "start" => {
            // 开始新执行
            let execution = self.executor.start(&input.name)
                .map_err(|e| ToolError::Execution(e.to_string()))?;

            let step_content = self.executor.get_current_step(&execution)
                .ok_or_else(|| ToolError::Execution("No steps found".into()))?;

            // 保存执行状态
            self.executions.write().insert(session_id.clone(), execution);

            Ok(format!(
                "Started skill '{}'. Session: {}\n\n{}\n\nRemaining steps:\n{}",
                input.name,
                session_id,
                step_content,
                self.executor.get_remaining_steps(&self.executions.read().get(&session_id).unwrap())
            ))
        }
        "continue" => {
            // 继续当前执行
            let mut executions = self.executions.write();
            let execution = executions.get_mut(&session_id)
                .ok_or_else(|| ToolError::Execution("No active execution for this session".into()))?;

            let step_content = self.executor.get_current_step(execution)
                .ok_or_else(|| ToolError::Execution("No more steps".into()))?;

            Ok(format!(
                "{}\n\nRemaining steps:\n{}",
                step_content,
                self.executor.get_remaining_steps(execution)
            ))
        }
        "complete" => {
            // 标记当前步骤完成
            let mut executions = self.executions.write();
            let execution = executions.get_mut(&session_id)
                .ok_or_else(|| ToolError::Execution("No active execution for this session".into()))?;

            self.executor.complete_step(execution);

            if execution.is_complete() {
                let summary = self.executor.get_progress_summary(execution);
                executions.remove(&session_id);
                Ok(format!("✅ {}", summary))
            } else {
                let step_content = self.executor.get_current_step(execution)
                    .map(|s| format!("{}\n\nRemaining steps:\n{}", s.task_name, s.content))
                    .unwrap_or_else(|| "All steps completed!".to_string());
                Ok(format!("{}\n\nRemaining:\n{}", self.executor.get_progress_summary(execution), step_content))
            }
        }
        "status" => {
            // 获取状态
            let executions = self.executions.read();
            if let Some(execution) = executions.get(&session_id) {
                Ok(self.executor.get_progress_summary(execution))
            } else {
                Ok("No active skill execution".to_string())
            }
        }
        _ => Err(ToolError::InvalidArgs(format!("Unknown action: {}", input.action)))
    }
}
```

- [ ] **Step 5: 更新 load_skill_registry_and_manager 函数**

```rust
pub fn load_skill_registry_and_manager() -> (Arc<RwLock<SkillRegistry>>, Arc<RwLock<SkillManager>>, Arc<SkillExecutor>) {
    // ... existing code ...
    let executor = Arc::new(SkillExecutor::from_default_dirs()
        .unwrap_or_else(|_| SkillExecutor::new(Arc::clone(&registry))));
    (registry, manager, executor)
}
```

- [ ] **Step 6: 运行 cargo check 验证**

Run: `cargo check -p hermes-tools-builtin`
Expected: 编译成功

- [ ] **Step 7: 提交**

```bash
git add crates/hermes-tools-builtin/src/skills.rs
git commit -m "feat(skills): 扩展 skill_execute 工具支持逐步执行"
```

---

## Task 6: 运行完整测试

- [ ] **Step 1: 运行 hermes-skills 测试**

Run: `cargo test -p hermes-skills 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 2: 运行 hermes-tools-builtin 测试**

Run: `cargo test -p hermes-tools-builtin 2>&1 | tail -20`
Expected: 所有测试通过

- [ ] **Step 3: 运行所有测试**

Run: `cargo test --all 2>&1 | tail -30`
Expected: 所有测试通过

- [ ] **Step 4: 提交最终变更**

```bash
git add -A
git commit -m "feat(skills): 完成技能主动执行功能

- SkillExecutor 解析技能 checkbox 步骤
- skill_execute 工具支持 start/complete/status 操作
- 会话级别状态跟踪
Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## 成功标准检查清单

- [ ] `parse_steps` 能正确解析 checkbox 语法
- [ ] `SkillExecutor::start` 返回技能第一步
- [ ] `skill_execute(name, action="start")` 返回技能第一步
- [ ] `skill_execute(name, action="complete")` 标记步骤完成并返回下一步
- [ ] `skill_execute(name, action="status")` 返回进度摘要
- [ ] 多个 Agent 调用之间保持状态（通过 session_id）
- [ ] 现有技能（writing-plans）可正常使用
