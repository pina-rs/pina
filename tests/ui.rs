//! Compile-failure and compile-pass UI tests for the Pina macros.
//!
//! `trybuild` compares the compiler's output against the checked-in `.stderr`
//! snapshots byte for byte. Instrumenting the crate under `cargo llvm-cov`
//! changes that output — the diagnostics gain notes about the instrumented
//! types — so two of the 43 cases mismatch under coverage even though the
//! macros are unchanged. The suite therefore does not run there, matching
//! `crates/pina_cli/tests/generated_surfpool.rs`. It still runs in the
//! post-merge and `ci-full` tiers via `test:all`, where the snapshots are the
//! real gate.
#![cfg(not(coverage))]

#[test]
fn macro_ui() {
	// Refresh the checked-in `.stderr` files with:
	// `TRYBUILD=overwrite cargo test -p pina_root --test ui -- --nocapture`
	let t = trybuild::TestCases::new();

	t.compile_fail("tests/ui/fail/*.rs");
	t.pass("tests/ui/pass/*.rs");
}
