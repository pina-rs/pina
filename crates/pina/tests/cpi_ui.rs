#[test]
fn cpi_context_requires_a_validated_program() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/cpi_context_rejects_raw_address.rs");
	tests.compile_fail("tests/ui/compact_rejects_inline_field_after_tail.rs");
	tests.compile_fail("tests/ui/compact_requires_dynamic_tail.rs");
}
