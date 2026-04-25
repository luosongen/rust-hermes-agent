#[cfg(test)]
mod compression_tests {
    use crate::compressed::CompressedSegment;

    #[test]
    fn test_compressed_segment_contains() {
        let segment = CompressedSegment::new(
            "session1".into(),
            1, 10,
            "Test summary".into(),
            vec![0.1, 0.2, 0.3],
        );

        assert!(segment.contains(5));
        assert!(!segment.contains(0));
        assert!(!segment.contains(11));
    }

    #[test]
    fn test_compressed_segment_range() {
        let segment = CompressedSegment::new(
            "session1".into(),
            5, 15,
            "Test summary".into(),
            vec![],
        );

        assert_eq!(segment.message_range(), (5, 15));
    }
}