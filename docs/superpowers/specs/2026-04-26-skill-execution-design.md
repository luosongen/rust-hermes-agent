# Skill 主动执行设计

> **Goal:** 让 Agent 能够主动遵循技能工作流，带进度跟踪和状态管理

> **Architecture:** 在现有 `skill_execute` 工具基础上添加状态跟踪层，解析技能中的 checkbox 步骤并逐步返回

> **Tech Stack:** Rust, tokio, parking_lot

---

## 1. 概述

### 1.1 当前状态

`skill_execute` 工具只是返回技能的完整内容，Agent 自行理解后执行。没有状态跟踪。

### 1.2 目标

1. **逐步执行** - Agent 调用 `skill_execute` 时，Executor 返回当前步骤而非全文
2. **状态跟踪** - 跟踪已完成的步骤，在会话中保持状态
3. **步骤标记** - 解析技能中的 checkbox 语法 `- [ ]` 标记的步骤

---

## 2. 架构设计

### 2.1 组件关系

```
┌─────────────────────────────────────────────────────┐
│                    Agent Loop                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │skill_execute│→ │SkillExecutor│→ │SkillState   │ │
│  │   Tool      │  │  (新)       │  │ (per-session)│ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│                           ↓                          │
│                    ┌─────────────┐                  │
│                    │ SkillLoader │                  │
│                    └─────────────┘                  │
└─────────────────────────────────────────────────────┘
```

### 2.2 技能格式约定

技能使用 checkbox 语法标记可执行步骤：

```markdown
---
name: writing-plans
description: Creates implementation plans
---

# Writing Plans

## Task 1: Analyze requirements
- [ ] Explore current project state
- [ ] Identify key files to modify
- [ ] Define success criteria

## Task 2: Write plan
- [ ] Create plan document
- [ ] Add implementation steps
```

### 2.3 SkillExecutor 接口

```rust
/// 技能执行状态（每个会话独立）
pub struct SkillExecution {
    skill_name: String,
    current_task: usize,      // 当前任务索引
    current_step: usize,      // 当前步骤索引
    completed_steps: Vec<(usize, usize)>,  // (task_idx, step_idx)
}

/// 技能执行器
pub struct SkillExecutor {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl SkillExecutor {
    /// 开始执行技能
    pub fn start(&self, skill_name: &str) -> Result<SkillExecution, SkillError>;

    /// 获取当前步骤内容
    pub fn get_current_step(&self, execution: &SkillExecution) -> Option<String>;

    /// 标记步骤完成
    pub fn complete_step(&self, execution: &mut SkillExecution);

    /// 获取剩余步骤摘要
    pub fn get_remaining_summary(&self, execution: &SkillExecution) -> String;
}
```

### 2.4 工具接口扩展

```rust
/// 扩展 skill_execute 工具参数
struct SkillExecuteInput {
    name: String,           // 技能名称
    action: String,         // "start" | "step" | "complete" | "status"
    step_content: Option<String>,  // 步骤执行后的结果
}

impl Tool for SkillExecuteTool {
    async fn execute(&self, args: Value, context: ToolContext) -> Result<String, ToolError> {
        let input: SkillExecuteInput = serde_json::from_value(args)?;

        match input.action.as_str() {
            "start" => {
                // 返回技能第一步
            }
            "step" => {
                // 返回当前步骤内容
            }
            "complete" => {
                // 标记当前步骤完成，返回下一步
            }
            "status" => {
                // 返回进度摘要
            }
        }
    }
}
```

---

## 3. 实现细节

### 3.1 步骤解析

从技能内容中提取步骤：

```rust
fn parse_steps(content: &str) -> Vec<(usize, usize, String)> {
    // 返回 (task_idx, step_idx, step_content)
    // 解析 "- [ ]" 行
}
```

### 3.2 状态存储

`SkillExecution` 存储在会话级别：
- 使用 `ToolContext` 中的会话 ID
- 或者存储在 `SessionStore` 中

### 3.3 Agent 交互流程

```
Agent: "使用 writing-plans 技能"
Tool: skill_execute(name="writing-plans", action="start")
    → 返回 "Task 1: Analyze requirements\n- [ ] Explore current project state..."

Agent: 完成探索，调用 "skill_execute(name='writing-plans', action='complete', step_content='...')"
Tool: 标记第一步完成，返回下一步

Agent: 继续直到所有步骤完成
```

---

## 4. 文件变更

| 文件 | 变更 |
|------|------|
| `crates/hermes-skills/src/executor.rs` | 新增：SkillExecutor 实现 |
| `crates/hermes-skills/src/lib.rs` | 导出 SkillExecutor |
| `crates/hermes-tools-builtin/src/skills.rs` | 修改：扩展 skill_execute 工具 |
| `crates/hermes-core/src/types.rs` | 可选：添加 SkillExecution 类型 |

---

## 5. 依赖

无新依赖。使用现有的：
- `parking_lot::RwLock`
- `hermes_skills::SkillLoader`
- `hermes_skills::SkillRegistry`

---

## 6. 成功标准

1. `skill_execute(name, action="start")` 返回技能第一步
2. `skill_execute(name, action="complete")` 标记步骤完成并返回下一步
3. `skill_execute(name, action="status")` 返回进度摘要
4. 多个 Agent 调用之间保持状态
5. 现有技能（writing-plans, subagent-driven-development）可正常使用
