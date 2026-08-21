#[test]
fn cpi_context_requires_a_validated_program() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/cpi_context_rejects_raw_address.rs");
}
