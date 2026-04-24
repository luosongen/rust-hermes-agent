//! Integration tests for Nudge system

use crate::{NudgeConfig, NudgeService, NudgeState, NudgeTrigger};

#[test]
fn test_memory_nudge_triggers_at_interval() {
    let config = NudgeConfig {
        memory_interval: 3,
        skill_interval: 10,
    };
    let service = NudgeService::new(config);
    let mut state = NudgeState::default();

    // Turns 1 and 2 - no trigger
    assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::None);
    assert_eq!(state.turns_since_memory, 1);

    assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::None);
    assert_eq!(state.turns_since_memory, 2);

    // Turn 3 - should trigger memory
    assert_eq!(
        service.check_triggers(&mut state, 1, 0),
        NudgeTrigger::Memory
    );
    assert_eq!(state.turns_since_memory, 0); // reset after trigger
}

#[test]
fn test_skill_nudge_triggers_at_interval() {
    let config = NudgeConfig {
        memory_interval: 10,
        skill_interval: 3,
    };
    let service = NudgeService::new(config);
    let mut state = NudgeState::default();

    // 2 tool calls total - no trigger
    assert_eq!(service.check_triggers(&mut state, 1, 2), NudgeTrigger::None);
    assert_eq!(state.iters_since_skill, 2);

    // 1 more tool call (total 3) - should trigger skill
    assert_eq!(
        service.check_triggers(&mut state, 1, 1),
        NudgeTrigger::Skill
    );
    assert_eq!(state.iters_since_skill, 0); // reset after trigger
}

#[test]
fn test_both_nudges_trigger_simultaneously() {
    let config = NudgeConfig {
        memory_interval: 2,
        skill_interval: 2,
    };
    let service = NudgeService::new(config);
    let mut state = NudgeState::default();

    // Turn 1 with 1 tool call
    assert_eq!(service.check_triggers(&mut state, 1, 1), NudgeTrigger::None);
    assert_eq!(state.turns_since_memory, 1);
    assert_eq!(state.iters_since_skill, 1);

    // Turn 2 with 1 tool call (both thresholds reached)
    assert_eq!(service.check_triggers(&mut state, 1, 1), NudgeTrigger::Both);
    assert_eq!(state.turns_since_memory, 0);
    assert_eq!(state.iters_since_skill, 0);
}

#[test]
fn test_disabled_nudge_never_triggers() {
    let config = NudgeConfig::disabled();
    let service = NudgeService::new(config);
    let mut state = NudgeState::default();

    // Even with many turns and tool calls, should never trigger when disabled
    for _ in 0..10 {
        assert_eq!(
            service.check_triggers(&mut state, 1, 100),
            NudgeTrigger::None
        );
    }
}

#[test]
fn test_nudge_state_resets_after_trigger() {
    let config = NudgeConfig {
        memory_interval: 2,
        skill_interval: 2,
    };
    let service = NudgeService::new(config);
    let mut state = NudgeState::default();

    // First call: turns=1, iters=1 - neither threshold met
    assert_eq!(service.check_triggers(&mut state, 1, 1), NudgeTrigger::None);

    // Second call: turns=2, iters=2 - both thresholds met, triggers Both
    assert_eq!(service.check_triggers(&mut state, 1, 1), NudgeTrigger::Both);
    assert_eq!(state.turns_since_memory, 0); // reset
    assert_eq!(state.iters_since_skill, 0); // reset

    // Next turn should not trigger (counters reset)
    assert_eq!(service.check_triggers(&mut state, 1, 0), NudgeTrigger::None);
}

#[test]
fn test_nudge_config_default_values() {
    let config = NudgeConfig::default();
    assert_eq!(config.memory_interval, 10);
    assert_eq!(config.skill_interval, 15);
}

#[test]
fn test_nudge_config_disabled_values() {
    let config = NudgeConfig::disabled();
    assert_eq!(config.memory_interval, 0);
    assert_eq!(config.skill_interval, 0);
}

#[test]
fn test_get_prompt_returns_correct_prompts() {
    use crate::ReviewPrompts;

    let service = NudgeService::new(NudgeConfig::default());

    assert_eq!(
        service.get_prompt(NudgeTrigger::Memory),
        ReviewPrompts::MEMORY_REVIEW
    );
    assert_eq!(
        service.get_prompt(NudgeTrigger::Skill),
        crate::nudge::ReviewPrompts::SKILL_REVIEW
    );
    assert_eq!(
        service.get_prompt(NudgeTrigger::Both),
        crate::nudge::ReviewPrompts::COMBINED_REVIEW
    );
}
