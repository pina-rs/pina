#[cfg(feature = "validation")]
#[test]
fn validation_annotations_reject_ambiguous_or_misspelled_rules() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_rejects_numeric_on_bool.rs");
	tests.compile_fail("tests/ui/validation_rejects_split_exact_len.rs");
	tests.compile_fail("tests/ui/validation_rejects_redundant_writable.rs");
	tests.compile_fail("tests/ui/validation_rejects_typo.rs");
}

/// The two halves of the spelling contract: the deprecated named bounds still
/// compile but warn, while the comparison rules are silent.
#[cfg(feature = "validation")]
#[test]
fn deprecated_bounds_warn_and_comparisons_do_not() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_deprecated_bounds.rs");
	tests.pass("tests/ui/validation_comparison_rules.rs");
}

#[cfg(not(feature = "validation"))]
#[test]
fn validation_annotations_explain_how_to_enable_the_feature() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/validation_requires_feature.rs");
}
