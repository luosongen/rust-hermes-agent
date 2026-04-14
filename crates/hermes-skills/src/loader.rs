use crate::error::SkillError;
use crate::metadata::SkillMetadata;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

/// A loaded skill with parsed content.
#[derive(Debug, Clone)]
pub struct Skill {
    pub metadata: SkillMetadata,
    /// The full body text after frontmatter (markdown content).
    pub content: String,
    /// Code blocks extracted from the skill body.
    pub code_blocks: Vec<CodeBlock>,
    /// Examples extracted from the skill body.
    pub examples: Vec<String>,
    /// Absolute path to the skill file.
    pub path: PathBuf,
}

/// A code block extracted from a skill.
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub lang: Option<String>,
    pub code: String,
}

impl Skill {
    /// Parse frontmatter from skill file content.
    fn parse_frontmatter(raw: &str) -> Result<(String, String), SkillError> {
        let trimmed = raw.trim_start();
        if !trimmed.starts_with("---") {
            return Err(SkillError::ParseFrontmatter(
                "Missing --- opening delimiter".into(),
            ));
        }
        let after_delim = &trimmed[3..];
        let end = after_delim
            .find("\n---")
            .ok_or_else(|| {
                SkillError::ParseFrontmatter("Missing closing --- delimiter".into())
            })?;
        let frontmatter = after_delim[..end].trim();
        let body = after_delim[end + 4..].trim().to_string();
        Ok((frontmatter.to_string(), body))
    }

    /// Load and parse a single skill file.
    pub fn from_path(path: &Path) -> Result<Self, SkillError> {
        let raw = fs::read_to_string(path)?;
        let (frontmatter, body) = Self::parse_frontmatter(&raw)?;
        let metadata: SkillMetadata =
            serde_yaml::from_str(&frontmatter)
                .map_err(|e| SkillError::ParseFrontmatter(e.to_string()))?;
        let code_blocks = Self::extract_code_blocks(&body);
        let examples = Self::extract_examples(&body);
        Ok(Self {
            metadata,
            content: body,
            code_blocks,
            examples,
            path: path.to_path_buf(),
        })
    }

    pub(crate) fn extract_code_blocks(body: &str) -> Vec<CodeBlock> {
        let re = Regex::new(r"```(\w*)\n([\s\S]*?)```").unwrap();
        re.captures_iter(body)
            .map(|cap| CodeBlock {
                lang: cap.get(1).map(|m| m.as_str().to_string()),
                code: cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
            })
            .collect()
    }

    pub(crate) fn extract_examples(body: &str) -> Vec<String> {
        let re = Regex::new(r"(?m)^/[\w-]+.*$").unwrap();
        re.find_iter(body)
            .map(|m| m.as_str().to_string())
            .collect()
    }
}

/// Loads skills from local directories.
pub struct SkillLoader {
    dirs: Vec<PathBuf>,
}

impl SkillLoader {
    pub fn new(dirs: Vec<PathBuf>) -> Self {
        Self { dirs }
    }

    /// Load all skills from all configured directories.
    pub fn load_all(&self) -> Result<Vec<Skill>, SkillError> {
        let mut skills = Vec::new();
        for dir in &self.dirs {
            skills.extend(self.load_from_dir(dir)?);
        }
        Ok(skills)
    }

    /// Load all skills from a single directory (non-recursive).
    pub fn load_from_dir(&self, dir: &Path) -> Result<Vec<Skill>, SkillError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut skills = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match Skill::from_path(&path) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!("Skipping invalid skill {:?}: {}", path, e);
                    }
                }
            }
        }
        Ok(skills)
    }

    /// Get the default skills directories (~/.hermes/skills, ./skills).
    pub fn default_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let default = home.join(".hermes/skills");
            if default.exists() || std::env::var("HERMES_SKILLS_HOME").is_ok() {
                dirs.push(default);
            }
        }
        if std::env::var("HERMES_SKILLS_LOCAL").is_ok() {
            dirs.push(PathBuf::from("skills"));
        }
        dirs
    }
}
