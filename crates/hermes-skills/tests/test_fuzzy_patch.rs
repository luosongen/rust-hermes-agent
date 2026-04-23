use hermes_skills::fuzzy_patch::FuzzyPatch;

#[test]
fn test_basic_patch() {
    let patch = FuzzyPatch::new();
    let content = "Hello World";
    let result = patch.patch(content, "World", "Rust");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Hello Rust");
}

#[test]
fn test_whitespace_flexibility() {
    let patch = FuzzyPatch::new();
    let content = "fn  foo() {\n    bar();\n}";
    // Should handle extra spaces
    let result = patch.patch(content, "fn foo() {", "fn bar() {");
    assert!(result.is_ok());
}

#[test]
fn test_no_match() {
    let patch = FuzzyPatch::new();
    let content = "Hello World";
    let result = patch.patch(content, "NotFound", "Replacement");
    assert!(result.is_err());
}