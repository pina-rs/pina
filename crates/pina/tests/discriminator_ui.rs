#[test]
fn reserved_migrate_discriminator_is_rejected() {
	let tests = trybuild::TestCases::new();
	tests.compile_fail("tests/ui/reserved_migrate_discriminator.rs");
}
