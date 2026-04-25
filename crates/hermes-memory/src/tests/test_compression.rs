#[cfg(test)]
mod compression_tests {
    use crate::compression_config::{CompressionConfig, CompressionMode, SummarizerConfig, SummarizerProvider};
    use crate::summarizer::Summarizer;
    use crate::SqliteSessionStore;
    use crate::compressed::CompressedSegment;
    use crate::compression::CompressionManager;
    use crate::session::SessionStore;
    use tempfile::tempdir;
    use std::sync::Arc;

    fn create_test_configs() -> (CompressionConfig, SummarizerConfig) {
        let compression = CompressionConfig {
            enabled: true,
            token_threshold: 1000,
            message_count_threshold: 10,
            min_compression_unit: 5,
            max_summary_tokens: 100,
            mode: CompressionMode::Hybrid,
        };

        let summarizer = SummarizerConfig {
            provider: SummarizerProvider::Ollama,
            model: "llama3".to_string(),
            ollama_url: Some("http://localhost:11434".to_string()),
        };

        (compression, summarizer)
    }

    #[tokio::test]
    async fn test_compression_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = SqliteSessionStore::new(db_path)
            .await
            .unwrap();
        let store = Arc::new(store);

        let (compression_config, summarizer_config) = create_test_configs();
        let summarizer = Summarizer::new(summarizer_config);
        let manager = CompressionManager::new(compression_config, summarizer, Arc::clone(&store));

        // Nonexistent session should not trigger compression
        let result = manager.should_compress("nonexistent").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_compress_empty_session() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = SqliteSessionStore::new(db_path)
            .await
            .unwrap();
        let store = Arc::new(store);

        // Create a session first
        store.create_session(crate::session::NewSession {
            id: "test-session".to_string(),
            source: "test".to_string(),
            user_id: None,
            model: Some("gpt-4".to_string()),
        }).await.unwrap();

        let (compression_config, summarizer_config) = create_test_configs();
        let summarizer = Summarizer::new(summarizer_config);
        let manager = CompressionManager::new(compression_config, summarizer, Arc::clone(&store));

        // Should fail due to not enough messages
        let result = manager.compress("test-session").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_compressed_segment_structure() {
        let segment = CompressedSegment::new(
            "session1".into(),
            1, 10,
            "Test summary".into(),
            vec![0.1, 0.2, 0.3],
        );

        assert!(segment.contains(5));
        assert!(!segment.contains(0));
        assert!(!segment.contains(11));
        assert_eq!(segment.message_range(), (1, 10));
    }

    #[tokio::test]
    async fn test_should_compress_respects_disabled() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = SqliteSessionStore::new(db_path)
            .await
            .unwrap();
        let store = Arc::new(store);

        // Create a session with messages
        store.create_session(crate::session::NewSession {
            id: "test-session".to_string(),
            source: "test".to_string(),
            user_id: None,
            model: Some("gpt-4".to_string()),
        }).await.unwrap();

        // Add enough messages
        for i in 0..15 {
            store.append_message("test-session", crate::session::NewMessage {
                role: "user".into(),
                content: Some(format!("Message {}", i)),
                tool_call_id: None,
                tool_calls: None,
                tool_name: None,
                timestamp: 0.0,
                token_count: Some(100),
                finish_reason: None,
                reasoning: None,
            }).await.unwrap();
        }

        // Create manager with compression disabled
        let mut config = create_test_configs().0;
        config.enabled = false;
        let summarizer = Summarizer::new(create_test_configs().1);
        let manager = CompressionManager::new(config, summarizer, Arc::clone(&store));

        // Should not trigger compression even with many messages
        let result = manager.should_compress("test-session").await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn test_get_compressed_segments_empty() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let store = SqliteSessionStore::new(db_path)
            .await
            .unwrap();
        let store = Arc::new(store);

        // Create a session
        store.create_session(crate::session::NewSession {
            id: "test-session".to_string(),
            source: "test".to_string(),
            user_id: None,
            model: Some("gpt-4".to_string()),
        }).await.unwrap();

        let segments = store.get_compressed_segments("test-session").await.unwrap();
        assert!(segments.is_empty());
    }
}
