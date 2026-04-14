use crate::loader::Skill;
use crate::SkillRegistry;
use std::fs;
use tempfile::TempDir;

fn make_skill(name: &str) -> Skill {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("s.md");
    let raw = format!(r#"---
name: {}
description: A skill
---

Content.
"#, name);
    fs::write(&path, raw).unwrap();
    Skill::from_path(&path).unwrap()
}

#[test]
fn test_register_and_get() {
    let mut reg = SkillRegistry::new();
    let skill = make_skill("my-skill");
    reg.register(skill).unwrap();
    assert!(reg.get("my-skill").is_some());
    assert!(reg.get("missing").is_none());
}

#[test]
fn test_register_duplicate_error() {
    let mut reg = SkillRegistry::new();
    let s1 = make_skill("dup");
    let s2 = make_skill("dup");
    reg.register(s1).unwrap();
    let err = reg.register(s2).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn test_search() {
    let mut reg = SkillRegistry::new();
    let s1 = make_skill("rust-format");
    let s2 = make_skill("python-test");
    reg.register(s1).unwrap();
    reg.register(s2).unwrap();
    let results = reg.search("rust");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].metadata.name, "rust-format");
}

#[test]
fn test_names() {
    let mut reg = SkillRegistry::new();
    reg.register(make_skill("a")).unwrap();
    reg.register(make_skill("b")).unwrap();
    let mut names = reg.names();
    names.sort();
    assert_eq!(names, vec!["a", "b"]);
}
