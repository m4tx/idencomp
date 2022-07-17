#[test]
fn test_model() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/model-simple.rs");
    t.compile_fail("tests/ui/model-nonexistent-item.rs");
}
