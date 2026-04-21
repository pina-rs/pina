#[test]
fn macro_ui() {
	// Refresh the checked-in `.stderr` files with:
	// `TRYBUILD=overwrite cargo test -p pina_macros --test ui -- --nocapture`
	let t = trybuild::TestCases::new();

	t.compile_fail("tests/ui/fail/*.rs");
	t.pass("tests/ui/pass/*.rs");
}
