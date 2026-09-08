//! Integration tests for machine-applicable lint fixes.
//!
//! Each fixable lint is exercised through the same flow `pina lint --fix`
//! uses: `cargo fix` with the bundled `pina_lint_driver` as the workspace
//! rustc wrapper. The rewritten source must match the expectation exactly —
//! proving the suggestion did not mangle the code — and must recompile
//! without triggering the lint again.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Unfixed fixture source: every construct the unused-guard fix handles.
const UNFIXED_SOURCE: &str = r#"#![allow(dead_code, unused_variables, private_interfaces)]

struct AccountView;
struct Guard;
struct MintView;

impl AccountView {
	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
	}

	fn try_borrow(&self) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

trait AsTokenAccount {
	fn as_token_mint_for_program(&self, program: &u8) -> Result<Guard, ()>;
}

impl AsTokenAccount for MintView {
	fn as_token_mint_for_program(&self, _program: &u8) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

pub fn dropped_after_binding(account: &mut AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow_mut()?;
	drop(guard);
	Ok(0)
}

pub fn underscore_binding(mint: &MintView) -> Result<(), ()> {
	let _guard = mint.as_token_mint_for_program(&0)?;
	Ok(())
}

pub fn read_guard(account: &AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow()?;
	Ok(guard.arity())
}

impl Guard {
	pub fn arity(&self) -> u8 {
		0
	}
}
"#;

/// The exact source expected after `cargo fix`.
///
/// The underscore binding is rewritten into its initializer expression — the
/// only machine-applicable suggestion for this fixture. The drop-removal
/// suggestion is `MaybeIncorrect` (removing a later `drop(local);` releases
/// the borrow earlier than the user wrote it, and intervening code can observe
/// the held borrow), so cargo fix leaves `dropped_after_binding` untouched and
/// its warning remains for manual review. The read guard is untouched.
const FIXED_SOURCE: &str = r#"#![allow(dead_code, unused_variables, private_interfaces)]

struct AccountView;
struct Guard;
struct MintView;

impl AccountView {
	fn try_borrow_mut(&mut self) -> Result<Guard, ()> {
		Ok(Guard)
	}

	fn try_borrow(&self) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

trait AsTokenAccount {
	fn as_token_mint_for_program(&self, program: &u8) -> Result<Guard, ()>;
}

impl AsTokenAccount for MintView {
	fn as_token_mint_for_program(&self, _program: &u8) -> Result<Guard, ()> {
		Ok(Guard)
	}
}

pub fn dropped_after_binding(account: &mut AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow_mut()?;
	drop(guard);
	Ok(0)
}

pub fn underscore_binding(mint: &MintView) -> Result<(), ()> {
	mint.as_token_mint_for_program(&0)?;
	Ok(())
}

pub fn read_guard(account: &AccountView) -> Result<u8, ()> {
	let guard = account.try_borrow()?;
	Ok(guard.arity())
}

impl Guard {
	pub fn arity(&self) -> u8 {
		0
	}
}
"#;

/// The sysroot of the toolchain pinned by the workspace's `rust-toolchain.toml`.
///
/// The lint driver links against that toolchain's `rustc_private` libraries,
/// so the scratch crate must compile with the same toolchain even though it
/// lives outside the repository (where rustup cannot resolve the override).
/// Resolving through `rustc --print sysroot` from the workspace root keeps the
/// test independent of how the toolchain is named or installed.
fn pinned_sysroot() -> PathBuf {
	let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
	let workspace_root = manifest
		.ancestors()
		.nth(2)
		.expect("crate manifest must have a workspace root");

	let output = Command::new("rustc")
		.arg("--print")
		.arg("sysroot")
		.current_dir(workspace_root)
		.output()
		.expect("could not resolve the pinned toolchain sysroot");
	assert!(
		output.status.success(),
		"rustc --print sysroot failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let sysroot = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
	assert!(
		sysroot.is_dir(),
		"sysroot is missing: {}",
		sysroot.display()
	);
	sysroot
}

/// Environment variables pinning the scratch crate to the workspace toolchain.
///
/// The scratch crate lives in a temporary directory, so rustup's
/// directory-based toolchain resolution cannot find the workspace override:
/// every spawned process must pin the toolchain and its home explicitly.
fn pinned_toolchain_env() -> Vec<(String, String)> {
	let sysroot = pinned_sysroot();
	let toolchain_dir = sysroot
		.file_name()
		.expect("sysroot must end in its toolchain directory")
		.to_string_lossy()
		.to_string();
	let rustup_home = sysroot
		.parent()
		.and_then(Path::parent)
		.expect("sysroot must live under a rustup home")
		.display()
		.to_string();
	let rustc = sysroot.join("bin").join("rustc");

	vec![
		("RUSTUP_TOOLCHAIN".to_string(), toolchain_dir),
		("RUSTUP_HOME".to_string(), rustup_home),
		("RUSTC".to_string(), rustc.display().to_string()),
	]
}

/// A scratch crate directory outside any workspace.
struct ScratchCrate {
	root: PathBuf,
}

impl ScratchCrate {
	fn new(name: &str, source: &str) -> Self {
		let root = std::env::temp_dir()
			.join("pina-lints-fix-tests")
			.join(format!("{name}-{}", std::process::id()));
		let _ = fs::remove_dir_all(&root);
		fs::create_dir_all(root.join("src")).expect("could not create scratch crate");

		fs::write(
			root.join("Cargo.toml"),
			r#"[package]
name = "fix_fixture"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]
"#,
		)
		.expect("could not write Cargo.toml");
		fs::write(root.join("src").join("lib.rs"), source).expect("could not write scratch source");

		Self { root }
	}

	fn source(&self) -> String {
		fs::read_to_string(self.root.join("src").join("lib.rs"))
			.expect("could not read scratch source")
	}

	fn cargo(&self, args: &[&str]) {
		let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
		let output = Command::new(&cargo)
			.args(args)
			.current_dir(&self.root)
			.envs(pinned_toolchain_env())
			// `cargo fix` runs primary units through a cargo-as-rustc proxy that
			// applies `RUSTC_WORKSPACE_WRAPPER` but bypasses `RUSTC_WRAPPER`, so
			// the fix flow only sees the lints through the workspace wrapper.
			.env(
				"RUSTC_WORKSPACE_WRAPPER",
				env!("CARGO_BIN_EXE_pina_lint_driver"),
			)
			.env("PINA_LINT_NO_DEPS", "1")
			.env("PINA_LINT_ONLY", "deny_unused_account_borrow_guards")
			.env("CARGO_TARGET_DIR", self.root.join("target"))
			.output()
			.expect("could not run cargo");
		assert!(
			output.status.success(),
			"cargo {args:?} failed while fixing the scratch crate:\n{}",
			String::from_utf8_lossy(&output.stderr)
		);
	}

	fn lint_warnings(&self) -> usize {
		let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
		let output = Command::new(&cargo)
			.args(["check", "--lib", "--offline"])
			.current_dir(&self.root)
			.envs(pinned_toolchain_env())
			.env(
				"RUSTC_WORKSPACE_WRAPPER",
				env!("CARGO_BIN_EXE_pina_lint_driver"),
			)
			.env("PINA_LINT_NO_DEPS", "1")
			.env("PINA_LINT_ONLY", "deny_unused_account_borrow_guards")
			.env("CARGO_TARGET_DIR", self.root.join("target"))
			.output()
			.expect("could not run cargo check");
		assert!(
			output.status.success(),
			"fixed scratch crate must still compile: {}",
			String::from_utf8_lossy(&output.stderr)
		);

		String::from_utf8_lossy(&output.stderr)
			.lines()
			.filter(|line| line.contains("account borrow guard"))
			.count()
	}
}

impl Drop for ScratchCrate {
	fn drop(&mut self) {
		let _ = fs::remove_dir_all(&self.root);
	}
}

#[test]
fn machine_applicable_fixes_recompile_without_mangling() {
	let scratch = ScratchCrate::new("unused-guards", UNFIXED_SOURCE);

	scratch.cargo(&[
		"fix",
		"--lib",
		"--offline",
		"--allow-dirty",
		"--allow-staged",
		"--allow-no-vcs",
	]);

	// The rewritten source must match the expectation exactly: every applied
	// suggestion is byte-for-byte what the lint advertised.
	assert_eq!(
		scratch.source(),
		FIXED_SOURCE,
		"the applied fix mangled the scratch crate"
	);

	// The fixed source still compiles. The machine-applicable fix is gone;
	// the MaybeIncorrect drop-removal warning intentionally remains for the
	// user to review by hand.
	assert_eq!(
		scratch.lint_warnings(),
		1,
		"only the MaybeIncorrect drop-removal warning should remain"
	);
}
