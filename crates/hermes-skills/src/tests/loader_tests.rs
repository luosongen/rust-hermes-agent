use crate::loader::Skill;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_parse_frontmatter_valid() {
    let raw = r#"---
name: test-skill
description: A test skill
platforms: [macos, linux]
---

# Test Skill

Some content here.
"#;
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("test.md");
    fs::write(&path, raw).unwrap();
    let skill = Skill::from_path(&path).unwrap();
    assert_eq!(skill.metadata.name, "test-skill");
    assert_eq!(skill.metadata.description, "A test skill");
    assert!(!skill.metadata.supports_platform("windows"));
    assert!(skill.metadata.supports_platform("macos"));
}

#[test]
fn test_parse_frontmatter_missing_delimiter() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.md");
    fs::write(&path, "name: no-frontmatter\n---\ncontent").unwrap();
    let err = Skill::from_path(&path).unwrap_err();
    assert!(err.to_string().contains("Missing ---"));
}

#[test]
fn test_extract_code_blocks() {
    let body = r#"
Some text.

```bash
echo hello
```

```python
print("world")
```
"#;
    let blocks = Skill::extract_code_blocks(body);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[0].lang.as_deref(), Some("bash"));
    assert_eq!(blocks[0].code.trim(), "echo hello");
    assert_eq!(blocks[1].lang.as_deref(), Some("python"));
}

#[test]
fn test_extract_examples() {
    let body = r#"
# Examples

/test-skill arg1 arg2
/another one
"#;
    let examples = Skill::extract_examples(body);
    assert_eq!(examples.len(), 2);
    assert_eq!(examples[0], "/test-skill arg1 arg2");
}
