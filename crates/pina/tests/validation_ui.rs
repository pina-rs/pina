#[cfg(feature = "validation")]
#[test]
fn validation_annotations_reject_ambiguous_or_misspelled_rules() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_rejects_numeric_on_bool.rs");
	tests.compile_fail("tests/ui/validation_rejects_redundant_writable.rs");
	tests.compile_fail("tests/ui/validation_rejects_typo.rs");
}

#[cfg(not(feature = "validation"))]
#[test]
fn validation_annotations_explain_how_to_enable_the_feature() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_requires_feature.rs");
}
