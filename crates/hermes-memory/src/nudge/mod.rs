//! Nudge system - periodic background memory review

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct NudgeConfig {
    pub memory_nudge_interval: usize,
    pub skill_nudge_interval: usize,
}

impl Default for NudgeConfig {
    fn default() -> Self {
        Self {
            memory_nudge_interval: 10,
            skill_nudge_interval: 10,
        }
    }
}

pub struct NudgeState {
    pub turns_since_memory: AtomicUsize,
    pub turns_since_skill: AtomicUsize,
}

impl Default for NudgeState {
    fn default() -> Self {
        Self::new()
    }
}

impl NudgeState {
    pub fn new() -> Self {
        Self {
            turns_since_memory: AtomicUsize::new(0),
            turns_since_skill: AtomicUsize::new(0),
        }
    }

    pub fn on_user_turn(&self) {
        self.turns_since_memory.fetch_add(1, Ordering::SeqCst);
        self.turns_since_skill.fetch_add(1, Ordering::SeqCst);
    }

    pub fn should_nudge_memory(&self, interval: usize) -> bool {
        self.turns_since_memory.load(Ordering::SeqCst) >= interval
    }

    pub fn should_nudge_skill(&self, interval: usize) -> bool {
        self.turns_since_skill.load(Ordering::SeqCst) >= interval
    }

    pub fn reset_memory(&self) {
        self.turns_since_memory.store(0, Ordering::SeqCst);
    }

    pub fn reset_skill(&self) {
        self.turns_since_skill.store(0, Ordering::SeqCst);
    }
}

pub trait NudgeExecutor: Send + Sync {
    fn execute_memory_review(&self, conversation_history: &str) -> Result<(), String>;
    fn execute_skill_review(&self, conversation_history: &str) -> Result<(), String>;
}