#[cfg(test)]
mod tests {
    use crate::SkillManager;
    use tempfile::TempDir;

    #[test]
    fn test_validate_name() {
        assert!(SkillManager::validate_name("valid-name").is_ok());
        assert!(SkillManager::validate_name("valid_name").is_ok());
        assert!(SkillManager::validate_name("abc123").is_ok());
        assert!(SkillManager::validate_name("").is_err());
        assert!(SkillManager::validate_name("Invalid").is_err()); // 大写不允许
        assert!(SkillManager::validate_name("-invalid").is_err()); // 不能以连字符开头
        assert!(SkillManager::validate_name("a".repeat(65).as_str()).is_err()); // 超过64字符
    }

    #[test]
    fn test_validate_category() {
        assert!(SkillManager::validate_category("").is_ok()); // 空是允许的
        assert!(SkillManager::validate_category("devops").is_ok());
        assert!(SkillManager::validate_category("data-science").is_ok());
        assert!(SkillManager::validate_category("Invalid").is_err());
    }

    #[test]
    fn test_validate_frontmatter_valid() {
        let valid = "---\nname: test\ndescription: test desc\n---\n# Content";
        assert!(SkillManager::validate_frontmatter(valid).is_ok());
    }

    #[test]
    fn test_validate_frontmatter_missing_delimiter() {
        let invalid_no_frontmatter = "# Just content";
        assert!(SkillManager::validate_frontmatter(invalid_no_frontmatter).is_err());
    }

    #[test]
    fn test_validate_frontmatter_empty_body() {
        let invalid_empty_body = "---\nname: test\ndescription: test\n---";
        assert!(SkillManager::validate_frontmatter(invalid_empty_body).is_err());
    }

    #[test]
    fn test_validate_file_path_valid() {
        assert!(SkillManager::validate_file_path("references/api.md").is_ok());
        assert!(SkillManager::validate_file_path("templates/config.yaml").is_ok());
        assert!(SkillManager::validate_file_path("scripts/run.sh").is_ok());
        assert!(SkillManager::validate_file_path("assets/logo.png").is_ok());
    }

    #[test]
    fn test_validate_file_path_traversal() {
        assert!(SkillManager::validate_file_path("../etc/passwd").is_err());
        assert!(SkillManager::validate_file_path("references/../../etc/passwd").is_err());
    }

    #[test]
    fn test_validate_file_path_invalid_subdir() {
        assert!(SkillManager::validate_file_path("invalid_subdir/file.md").is_err());
    }

    #[test]
    fn test_create_and_find_skill() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Test";
        let result = manager.create("test-skill", content, None);
        assert!(result.is_ok());

        let found = manager.find_skill_dir("test-skill");
        assert!(found.is_some());
    }

    #[test]
    fn test_create_with_category() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: my-skill\ndescription: test\n---\n# Content";
        let result = manager.create("my-skill", content, Some("devops"));
        assert!(result.is_ok());

        let found = manager.find_skill_dir("my-skill");
        assert!(found.is_some());
    }

    #[test]
    fn test_create_duplicate_fails() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Content";
        assert!(manager.create("test-skill", content, None).is_ok());
        assert!(manager.create("test-skill", content, None).is_err()); // 已存在
    }

    #[test]
    fn test_edit_skill() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Content";
        manager.create("test-skill", content, None).unwrap();

        let new_content = "---\nname: test\ndescription: updated\n---\n# Updated";
        let result = manager.edit("test-skill", new_content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_edit_nonexistent_fails() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Content";
        assert!(manager.edit("nonexistent", content).is_err());
    }

    #[test]
    fn test_delete_skill() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Content";
        manager.create("test-skill", content, None).unwrap();

        let result = manager.delete("test-skill");
        assert!(result.is_ok());

        let found = manager.find_skill_dir("test-skill");
        assert!(found.is_none());
    }

    #[test]
    fn test_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Content";
        manager.create("test-skill", content, None).unwrap();

        let result = manager.write_file("test-skill", "references/api.md", "# API Docs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Content";
        manager.create("test-skill", content, None).unwrap();
        manager.write_file("test-skill", "references/api.md", "# API Docs").unwrap();

        let result = manager.remove_file("test-skill", "references/api.md");
        assert!(result.is_ok());
    }

    #[test]
    fn test_patch_skill() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Old Content";
        manager.create("test-skill", content, None).unwrap();

        let result = manager.patch("test-skill", "Old", "New", false, None);
        assert!(result.is_ok());

        // 验证内容已更新
        let skill_dir = manager.find_skill_dir("test-skill").unwrap();
        let new_content = std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert!(new_content.contains("New"));
        assert!(!new_content.contains("Old"));
    }

    #[test]
    fn test_patch_replace_all() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SkillManager::with_dir(temp_dir.path().to_path_buf());

        let content = "---\nname: test\ndescription: test\n---\n# Foo Foo Foo";
        manager.create("test-skill", content, None).unwrap();

        let result = manager.patch("test-skill", "Foo", "Bar", true, None);
        assert!(result.is_ok());

        // 验证所有 Foo 都被替换
        let skill_dir = manager.find_skill_dir("test-skill").unwrap();
        let new_content = std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
        assert!(new_content.contains("Bar Bar Bar"));
    }
}