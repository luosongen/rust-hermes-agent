use serde::{Deserialize, Serialize};

/// Configuration item defined in skill frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfigItem {
    pub key: String,
    pub description: String,
    #[serde(default)]
    pub default: Option<String>,
}

/// Hermes-specific metadata inside the YAML frontmatter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HermesMetadata {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub config: Vec<SkillConfigItem>,
    #[serde(default)]
    pub requires_toolsets: Vec<String>,
}

/// The YAML frontmatter of a skill file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub metadata: HermesMetadata,
}

impl SkillMetadata {
    /// Returns true if this skill supports the given platform.
    pub fn supports_platform(&self, platform: &str) -> bool {
        self.platforms.is_empty() || self.platforms.iter().any(|p| p == platform)
    }

    /// Returns true if the skill requires the given toolset.
    pub fn requires_toolset(&self, toolset: &str) -> bool {
        self.metadata
            .requires_toolsets
            .iter()
            .any(|t| t == toolset)
    }
}
