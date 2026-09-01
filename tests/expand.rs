//! Macro expansion snapshot tests using `macrotest`.
//!
//! Each fixture file under `tests/expand/` invokes the Pina macros through
//! the public API exactly as a downstream consumer would. `macrotest` runs
//! `cargo expand` on each fixture and compares the expansion against the
//! checked-in `*.expanded.rs` snapshot, giving shape coverage for the
//! generated code without touching private internals.
//!
//! # Updating snapshots
//!
//! Run tests with `MACROTEST=overwrite` or delete the `*.expanded.rs` files
//! and re-run. Files are generated automatically.

#[test]
fn expansion_snapshots() {
	macrotest::expand("tests/expand/*.rs");
}
