//! Nudge System — 后台记忆/技能审查系统
//!
//! 本模块实现了定期触发后台审查的机制，用于提醒 Agent 保存记忆或更新技能。
//!
//! ## 主要类型
//! - **NudgeTrigger**: 触发类型枚举（无/记忆/技能/两者）
//! - **NudgeConfig**: Nudge 配置（审查间隔）
//! - **NudgeState**: Nudge 状态追踪
//! - **NudgeService**: Nudge 服务核心逻辑

use serde::{Deserialize, Serialize};

/// Nudge 触发类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NudgeTrigger {
    /// 不触发
    None,
    /// 触发记忆审查
    Memory,
    /// 触发技能审查
    Skill,
    /// 同时触发记忆和技能审查
    Both,
}

/// Nudge 配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NudgeConfig {
    /// 记忆审查间隔（对话轮数）
    pub memory_interval: usize,
    /// 技能审查间隔（工具调用次数）
    pub skill_interval: usize,
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            memory_interval: 10,
            skill_interval: 15,
        }
    }
}

impl NudgeConfig {
    /// 创建禁用的配置
    pub fn disabled() -> Self {
        Self {
            memory_interval: 0,
            skill_interval: 0,
        }
    }

    /// 获取配置值
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "memory_interval" => Some(self.memory_interval.to_string()),
            "skill_interval" => Some(self.skill_interval.to_string()),
            _ => None,
        }
    }
}

/// Nudge 状态追踪
#[derive(Debug, Clone)]
pub struct NudgeState {
    /// 距离上次记忆审查的对话轮数
    pub turns_since_memory: usize,
    /// 距离上次技能审查的工具调用次数
    pub iters_since_skill: usize,
}

impl Default for NudgeState {
    fn default() -> Self {
        Self {
            turns_since_memory: 0,
            iters_since_skill: 0,
        }
    }
}

/// 审查提示词常量
pub struct ReviewPrompts;

impl ReviewPrompts {
    /// 记忆审查提示词
    pub const MEMORY_REVIEW: &'static str = concat!(
        "Review the conversation above and consider saving to memory if appropriate.\n",
        "Focus on:\n",
        "1. Has the user revealed things about themselves — their persona, desires, ",
        "preferences, or personal details worth remembering?\n",
        "2. Has the user expressed expectations about how you should behave, ",
        "their work style, or ways they want you to operate?\n\n",
        "If something stands out, save it using the memory tool. ",
        "If nothing is worth saving, just say 'Nothing to save.' and stop."
    );

    /// 技能审查提示词
    pub const SKILL_REVIEW: &'static str = concat!(
        "Review the conversation above and consider saving or updating a skill if appropriate.\n\n",
        "Focus on: was a non-trivial approach used to complete a task that required trial ",
        "and error, or changing course due to experiential findings along the way, or did ",
        "the user expect or desire a different method or outcome?\n\n",
        "If a relevant skill already exists, update it with what you learned. ",
        "Otherwise, create a new skill if the approach is reusable.\n",
        "If nothing is worth saving, just say 'Nothing to save.' and stop."
    );

    /// 组合审查提示词（记忆 + 技能）
    pub const COMBINED_REVIEW: &'static str = concat!(
        "Review the conversation above and consider two things:\n\n",
        "**Memory**: Has the user revealed things about themselves — their persona, ",
        "desires, preferences, or personal details? Has the user expressed expectations ",
        "about how you should behave, their work style, or ways they want you to operate? ",
        "If so, save using the memory tool.\n\n",
        "**Skills**: Was a non-trivial approach used to complete a task that required trial ",
        "and error, or changing course due to experiential findings along the way? If a relevant skill ",
        "already exists, update it. Otherwise, create a new one if the approach is reusable.\n\n",
        "Only act if there's something genuinely worth saving. ",
        "If nothing stands out, just say 'Nothing to save.' and stop."
    );
}

/// Nudge 服务
///
/// 负责检查触发条件并提供审查提示词。
#[derive(Debug, Clone)]
pub struct NudgeService {
    config: NudgeConfig,
}

impl NudgeService {
    /// 创建新的 Nudge 服务
    pub fn new(config: NudgeConfig) -> Self {
        Self { config }
    }

    /// 检查是否应触发审查
    pub fn check_triggers(
        &self,
        state: &mut NudgeState,
        _user_turn_count: usize,
        tool_calls_this_turn: usize,
    ) -> NudgeTrigger {
        state.turns_since_memory += 1;
        state.iters_since_skill += tool_calls_this_turn;

        let memory_triggered = self.config.memory_interval > 0
            && state.turns_since_memory >= self.config.memory_interval;

        let skill_triggered =
            self.config.skill_interval > 0 && state.iters_since_skill >= self.config.skill_interval;

        if memory_triggered {
            state.turns_since_memory = 0;
        }
        if skill_triggered {
            state.iters_since_skill = 0;
        }

        match (memory_triggered, skill_triggered) {
            (true, true) => NudgeTrigger::Both,
            (true, false) => NudgeTrigger::Memory,
            (false, true) => NudgeTrigger::Skill,
            (false, false) => NudgeTrigger::None,
        }
    }

    /// 获取对应触发类型的审查提示词
    pub fn get_prompt(&self, trigger: NudgeTrigger) -> &'static str {
        match trigger {
            NudgeTrigger::Memory => ReviewPrompts::MEMORY_REVIEW,
            NudgeTrigger::Skill => ReviewPrompts::SKILL_REVIEW,
            NudgeTrigger::Both => ReviewPrompts::COMBINED_REVIEW,
            NudgeTrigger::None => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nudge_trigger_enum() {
        assert_eq!(NudgeTrigger::None, NudgeTrigger::None);
        assert_eq!(NudgeTrigger::Memory, NudgeTrigger::Memory);
        assert_eq!(NudgeTrigger::Skill, NudgeTrigger::Skill);
        assert_eq!(NudgeTrigger::Both, NudgeTrigger::Both);
        assert_ne!(NudgeTrigger::None, NudgeTrigger::Memory);
    }

    #[test]
    fn test_nudge_config_default() {
        let config = NudgeConfig::default();
        assert_eq!(config.memory_interval, 10);
        assert_eq!(config.skill_interval, 15);
    }

    #[test]
    fn test_nudge_config_disabled() {
        let config = NudgeConfig::disabled();
        assert_eq!(config.memory_interval, 0);
        assert_eq!(config.skill_interval, 0);
    }

    #[test]
    fn test_nudge_state_default() {
        let state = NudgeState::default();
        assert_eq!(state.turns_since_memory, 0);
        assert_eq!(state.iters_since_skill, 0);
    }

    #[test]
    fn test_nudge_service_check_triggers_memory() {
        let config = NudgeConfig {
            memory_interval: 3,
            skill_interval: 10,
        };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // First turn: threshold not reached
        let trigger = service.check_triggers(&mut state, 1, 0);
        assert_eq!(trigger, NudgeTrigger::None);
        assert_eq!(state.turns_since_memory, 1);

        // Second turn: threshold not reached
        let trigger = service.check_triggers(&mut state, 2, 0);
        assert_eq!(trigger, NudgeTrigger::None);
        assert_eq!(state.turns_since_memory, 2);

        // Third turn: memory triggers (turns_since_memory >= 3)
        let trigger = service.check_triggers(&mut state, 3, 0);
        assert_eq!(trigger, NudgeTrigger::Memory);
        assert_eq!(state.turns_since_memory, 0); // reset after trigger
    }

    #[test]
    fn test_nudge_service_check_triggers_skill() {
        let config = NudgeConfig {
            memory_interval: 10,
            skill_interval: 3,
        };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // First turn with 1 tool call
        let trigger = service.check_triggers(&mut state, 1, 1);
        assert_eq!(trigger, NudgeTrigger::None);
        assert_eq!(state.iters_since_skill, 1);

        // Second turn with 1 tool call
        let trigger = service.check_triggers(&mut state, 2, 1);
        assert_eq!(trigger, NudgeTrigger::None);
        assert_eq!(state.iters_since_skill, 2);

        // Third turn with 1 tool call: skill triggers (iters_since_skill >= 3)
        let trigger = service.check_triggers(&mut state, 3, 1);
        assert_eq!(trigger, NudgeTrigger::Skill);
        assert_eq!(state.iters_since_skill, 0); // reset after trigger
    }

    #[test]
    fn test_nudge_service_check_triggers_both() {
        let config = NudgeConfig {
            memory_interval: 3,
            skill_interval: 3,
        };
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // Turn 1
        service.check_triggers(&mut state, 1, 1);
        assert_eq!(state.turns_since_memory, 1);
        assert_eq!(state.iters_since_skill, 1);

        // Turn 2
        service.check_triggers(&mut state, 2, 1);
        assert_eq!(state.turns_since_memory, 2);
        assert_eq!(state.iters_since_skill, 2);

        // Turn 3: both trigger simultaneously
        let trigger = service.check_triggers(&mut state, 3, 1);
        assert_eq!(trigger, NudgeTrigger::Both);
        assert_eq!(state.turns_since_memory, 0);
        assert_eq!(state.iters_since_skill, 0);
    }

    #[test]
    fn test_nudge_service_disabled() {
        let config = NudgeConfig::disabled();
        let service = NudgeService::new(config);
        let mut state = NudgeState::default();

        // Even with many turns and tool calls, should never trigger when disabled
        for _ in 0..10 {
            let trigger = service.check_triggers(&mut state, 1, 100);
            assert_eq!(trigger, NudgeTrigger::None);
        }
    }

    #[test]
    fn test_get_prompt() {
        let config = NudgeConfig::default();
        let service = NudgeService::new(config);

        assert_eq!(
            service.get_prompt(NudgeTrigger::Memory),
            ReviewPrompts::MEMORY_REVIEW
        );
        assert_eq!(
            service.get_prompt(NudgeTrigger::Skill),
            ReviewPrompts::SKILL_REVIEW
        );
        assert_eq!(
            service.get_prompt(NudgeTrigger::Both),
            ReviewPrompts::COMBINED_REVIEW
        );
    }
}
