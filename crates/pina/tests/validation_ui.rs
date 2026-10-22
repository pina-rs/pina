#[cfg(feature = "validation")]
#[test]
fn validation_annotations_reject_misspelled_rules() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_rejects_numeric_on_bool.rs");
	tests.compile_fail("tests/ui/validation_rejects_redundant_writable.rs");
	tests.compile_fail("tests/ui/validation_rejects_typo.rs");
}

/// The comparison rules compile without diagnostics.
#[cfg(feature = "validation")]
#[test]
fn comparison_rules_are_silent() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/validation_comparison_rules.rs");
}

#[cfg(not(feature = "validation"))]
#[test]
fn validation_annotations_explain_how_to_enable_the_feature() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_requires_feature.rs");
}
