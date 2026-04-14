use crate::error::SkillError;
use crate::loader::Skill;
use std::collections::HashMap;

/// In-memory registry of loaded skills.
pub struct SkillRegistry {
    by_name: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }

    /// Register a skill. Returns error if a skill with the same name already exists.
    pub fn register(&mut self, skill: Skill) -> Result<(), SkillError> {
        let name = skill.metadata.name.clone();
        if self.by_name.contains_key(&name) {
            return Err(SkillError::AlreadyExists(name));
        }
        self.by_name.insert(name, skill);
        Ok(())
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.by_name.get(name)
    }

    /// List all skill names.
    pub fn names(&self) -> Vec<String> {
        self.by_name.keys().cloned().collect()
    }

    /// Search skills by name or description substring.
    pub fn search(&self, query: &str) -> Vec<&Skill> {
        let query_lower = query.to_lowercase();
        self.by_name
            .values()
            .filter(|s| {
                s.metadata.name.to_lowercase().contains(&query_lower)
                    || s.metadata.description.to_lowercase().contains(&query_lower)
            })
            .collect()
    }

    /// Total count of registered skills.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}
