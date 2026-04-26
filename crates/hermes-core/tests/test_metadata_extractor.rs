#[cfg(test)]
mod tests {
    use hermes_core::metadata_extractor::MetadataExtractor;
    use hermes_core::{Message, Content, Role};

    #[test]
    fn test_extract_file_refs() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: Content::Text("Read src/main.rs and Cargo.toml".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.file_refs.is_empty());
        assert!(metadata.file_refs.iter().any(|f| f.path.contains("main.rs")));
    }

    #[test]
    fn test_extract_no_files() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: Content::Text("Hello world".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(metadata.file_refs.is_empty());
    }

    #[test]
    fn test_extract_symbol_refs() {
        let extractor = MetadataExtractor::new();
        // Test: struct keyword followed by uppercase symbol name
        let messages = vec![
            Message {
                role: Role::Assistant,
                content: Content::Text("I created struct ContextCompressor".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.symbol_refs.is_empty());
    }

    #[test]
    fn test_extract_decisions() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::User,
                content: Content::Text("决定用 Arc<RwLock<T>> 来共享状态".to_string()),
                reasoning: None,
                tool_call_id: None,
                tool_name: None,
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.decisions.is_empty());
    }

    #[test]
    fn test_extract_tool_summaries() {
        let extractor = MetadataExtractor::new();
        let messages = vec![
            Message {
                role: Role::Tool,
                content: Content::ToolResult {
                    tool_call_id: "call_1".to_string(),
                    content: "File read successfully".to_string(),
                },
                reasoning: None,
                tool_call_id: Some("call_1".to_string()),
                tool_name: Some("ReadFile".to_string()),
            },
        ];

        let metadata = extractor.extract(&messages);
        assert!(!metadata.tool_summaries.is_empty());
        assert_eq!(metadata.tool_summaries[0].tool_name, "ReadFile");
    }

    #[test]
    fn test_extract_empty_messages() {
        let extractor = MetadataExtractor::new();
        let messages: Vec<Message> = vec![];

        let metadata = extractor.extract(&messages);
        assert!(metadata.file_refs.is_empty());
        assert!(metadata.symbol_refs.is_empty());
        assert!(metadata.decisions.is_empty());
        assert!(metadata.tool_summaries.is_empty());
    }
}