use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkillError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse frontmatter: {0}")]
    ParseFrontmatter(String),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Skill not found: {0}")]
    NotFound(String),

    #[error("Skill already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid skill path: {0}")]
    InvalidPath(String),

    #[error("Download failed: {0}")]
    Download(String),

    #[error("Skill is disabled on this platform: {0}")]
    PlatformNotSupported(String),
}
