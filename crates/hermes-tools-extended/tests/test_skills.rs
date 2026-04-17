use hermes_tools_extended::skills::SkillsTool;

#[test]
fn test_parse_skill_markdown() {
    let content = r#"---
name: test-skill
description: A test skill
triggers: ["test", "demo"]
tags: ["testing"]
---

# Test Skill

This is the skill content."#;

    let result = SkillsTool::parse_skill_markdown(content);
    assert!(result.is_some());
    let (meta, preview) = result.unwrap();
    assert_eq!(meta.name, "test-skill");
    assert_eq!(meta.description, "A test skill");
    assert_eq!(meta.triggers, vec!["test", "demo"]);
    assert!(preview.contains("skill content"));
}

#[test]
fn test_parse_missing_frontmatter() {
    let content = "# No frontmatter\nJust content";
    assert!(SkillsTool::parse_skill_markdown(content).is_none());
}

#[test]
fn test_parse_triggers_and_tags_default() {
    let content = r#"---
name: minimal-skill
description: Minimal skill without triggers or tags
---

Content here."#;

    let result = SkillsTool::parse_skill_markdown(content);
    assert!(result.is_some());
    let (meta, _) = result.unwrap();
    assert!(meta.triggers.is_empty());
    assert!(meta.tags.is_empty());
}