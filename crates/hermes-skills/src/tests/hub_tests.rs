#[cfg(test)]
mod tests {
    use crate::hub::{HubClient, HubError, SkillIndexEntry, SkillSource};
    use tempfile::TempDir;

    fn create_test_hub() -> Result<(HubClient, TempDir), HubError> {
        let temp = TempDir::new().unwrap();
        let home_dir = temp.path().to_path_buf();
        let hub = HubClient::new(home_dir)?;
        Ok((hub, temp))
    }

    #[tokio::test]
    async fn test_create_hub_client() {
        let result = create_test_hub();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_skills_empty() {
        let (hub, _temp) = create_test_hub().unwrap();
        let skills = hub.index.list_skills().unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_add_and_get_skill() {
        let (hub, _temp) = create_test_hub().unwrap();
        let entry = SkillIndexEntry {
            id: "test/skill".to_string(),
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            category: "test".to_string(),
            version: "1.0.0".to_string(),
            source: SkillSource::Local,
            checksum: "sha256:abc".to_string(),
            file_path: "/tmp/test.md".to_string(),
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        hub.index.add_skill(&entry).unwrap();
        let retrieved = hub.index.get_skill("test/skill").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-skill");
    }

    #[tokio::test]
    async fn test_remove_skill() {
        let (hub, _temp) = create_test_hub().unwrap();
        let entry = SkillIndexEntry {
            id: "test/skill".to_string(),
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            category: "test".to_string(),
            version: "1.0.0".to_string(),
            source: SkillSource::Local,
            checksum: "sha256:abc".to_string(),
            file_path: "/tmp/test.md".to_string(),
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        hub.index.add_skill(&entry).unwrap();
        hub.index.remove_skill("test/skill").unwrap();
        let retrieved = hub.index.get_skill("test/skill").unwrap();
        assert!(retrieved.is_none());
    }
}