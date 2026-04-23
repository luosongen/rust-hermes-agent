use hermes_skills::{skills_list, skills_view, skills_manage, SkillRegistry, SkillsListArgs, SkillsViewArgs, SkillsManageArgs};

#[test]
fn test_skills_list_empty() {
    let registry = SkillRegistry::new();
    let args = SkillsListArgs { category: None };
    let result = skills_list(&registry, args);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_skills_view_not_found() {
    let registry = SkillRegistry::new();
    let args = SkillsViewArgs { name: "nonexistent".to_string(), file_path: None };
    let result = skills_view(&registry, args);
    assert!(result.is_err());
}

#[test]
fn test_skills_manage_unknown_action() {
    let mut registry = SkillRegistry::new();
    let temp_dir = tempfile::tempdir().unwrap();
    let args = SkillsManageArgs {
        action: "unknown".to_string(),
        name: "test".to_string(),
        content: None,
        old_string: None,
        new_string: None,
    };
    let result = skills_manage(&mut registry, temp_dir.path(), args);
    assert!(result.is_err());
}

#[test]
fn test_skills_manage_create_and_list() {
    let mut registry = SkillRegistry::new();
    let temp_dir = tempfile::tempdir().unwrap();

    let content = r#"---
name: test-skill
description: A test skill
---
This is test content."#;

    let args = SkillsManageArgs {
        action: "create".to_string(),
        name: "test-skill".to_string(),
        content: Some(content.to_string()),
        old_string: None,
        new_string: None,
    };
    let result = skills_manage(&mut registry, temp_dir.path(), args);
    assert!(result.is_ok());

    let list_args = SkillsListArgs { category: None };
    let result = skills_list(&registry, list_args);
    assert!(result.is_ok());
    let skills = result.unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "test-skill");
}

#[test]
fn test_skills_manage_create_and_view() {
    let mut registry = SkillRegistry::new();
    let temp_dir = tempfile::tempdir().unwrap();

    let content = r#"---
name: view-test
description: Skill for viewing
---
View content here."#;

    let create_args = SkillsManageArgs {
        action: "create".to_string(),
        name: "view-test".to_string(),
        content: Some(content.to_string()),
        old_string: None,
        new_string: None,
    };
    skills_manage(&mut registry, temp_dir.path(), create_args).unwrap();

    let view_args = SkillsViewArgs { name: "view-test".to_string(), file_path: None };
    let result = skills_view(&registry, view_args);
    assert!(result.is_ok());
    let view = result.unwrap();
    assert_eq!(view.name, "view-test");
    assert!(view.content.contains("View content here"));
}

#[test]
fn test_skills_manage_delete() {
    let mut registry = SkillRegistry::new();
    let temp_dir = tempfile::tempdir().unwrap();

    let content = r#"---
name: delete-me
description: To be deleted
---
Delete me."#;

    let create_args = SkillsManageArgs {
        action: "create".to_string(),
        name: "delete-me".to_string(),
        content: Some(content.to_string()),
        old_string: None,
        new_string: None,
    };
    skills_manage(&mut registry, temp_dir.path(), create_args).unwrap();

    let delete_args = SkillsManageArgs {
        action: "delete".to_string(),
        name: "delete-me".to_string(),
        content: None,
        old_string: None,
        new_string: None,
    };
    let result = skills_manage(&mut registry, temp_dir.path(), delete_args);
    assert!(result.is_ok());

    let view_args = SkillsViewArgs { name: "delete-me".to_string(), file_path: None };
    let result = skills_view(&registry, view_args);
    assert!(result.is_err());
}

#[test]
fn test_skills_manage_patch() {
    let mut registry = SkillRegistry::new();
    let temp_dir = tempfile::tempdir().unwrap();

    let content = r#"---
name: patch-test
description: A skill for patching
---
fn hello() {
    println!("Hello World");
}"#;

    let create_args = SkillsManageArgs {
        action: "create".to_string(),
        name: "patch-test".to_string(),
        content: Some(content.to_string()),
        old_string: None,
        new_string: None,
    };
    skills_manage(&mut registry, temp_dir.path(), create_args).unwrap();

    // Verify initial content
    let result = skills_view(&registry, SkillsViewArgs { name: "patch-test".to_string(), file_path: None });
    assert!(result.is_ok());
    let view = result.unwrap();
    assert!(view.content.contains("Hello World"));

    // Patch the content
    let patch_args = SkillsManageArgs {
        action: "patch".to_string(),
        name: "patch-test".to_string(),
        content: None,
        old_string: Some("Hello World".to_string()),
        new_string: Some("Hello Rust".to_string()),
    };
    let result = skills_manage(&mut registry, temp_dir.path(), patch_args);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Skill 'patch-test' patched");

    // Verify patched content
    let result = skills_view(&registry, SkillsViewArgs { name: "patch-test".to_string(), file_path: None });
    assert!(result.is_ok());
    let view = result.unwrap();
    assert!(view.content.contains("Hello Rust"));
    assert!(!view.content.contains("Hello World"));
}