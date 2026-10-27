//! Integration tests for lint fix applicability.
//!
//! Each fixable lint is exercised through the same flow `pina lint --fix`
//! uses: `cargo fix` with the bundled `pina_lint_driver` as the workspace
//! rustc wrapper. Suggestions that shorten account lock lifetimes must leave
//! both the source and its runtime borrow behavior unchanged.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Fixture source whose named guards require manual review before removal.
const UNFIXED_SOURCE: &str = r#"#![allow(dead_code, unused_variables, private_interfaces)]

use solana_account_view::AccountView;
use solana_program_error::ProgramError;

pub fn dropped_after_binding(account: &mut AccountView) -> u8 {
	let guard = account.try_borrow_mut().unwrap();
	drop(guard);
	0
}

pub fn underscore_binding(account: &AccountView) {
	let _guard = account.try_borrow().unwrap();
}

pub fn read_guard(account: &AccountView) -> usize {
	let guard = account.try_borrow().unwrap();
	guard.len()
}

fn write_alias(alias: &mut AccountView) -> Result<(), ProgramError> {
	alias.try_borrow_mut()?[0] = 42;

	Ok(())
}

fn shared_lock(account: &mut AccountView, alias: &mut AccountView) -> Result<(), ProgramError> {
	let _guard = account.try_borrow()?;
	write_alias(alias)
}

fn exclusive_lock(account: &mut AccountView, alias: &mut AccountView) -> Result<(), ProgramError> {
	let _guard = account.try_borrow_mut()?;
	write_alias(alias)
}

fn explicit_drop(account: &mut AccountView, alias: &mut AccountView) -> Result<(), ProgramError> {
	let guard = account.try_borrow_mut()?;
	let result = write_alias(alias);
	drop(guard);
	result
}

#[test]
fn named_guards_block_alias_mutation() {
	use solana_account_view::RuntimeAccount;

	#[repr(C)]
	struct Account {
		header: RuntimeAccount,
		data: [u8; 1],
	}

	for lock in [shared_lock, exclusive_lock, explicit_drop] {
		let mut backing = Box::new(Account {
			header: RuntimeAccount {
				borrow_state: solana_account_view::NOT_BORROWED,
				is_writable: 1,
				data_len: 1,
				..RuntimeAccount::default()
			},
			data: [7],
		});

		// SAFETY: The stable allocation has a RuntimeAccount immediately
		// followed by data_len bytes and outlives both views and all guards.
		let mut account = unsafe { AccountView::new_unchecked((&raw mut *backing).cast()) };
		let mut alias = account.clone();

		assert_eq!(
			lock(&mut account, &mut alias),
			Err(ProgramError::AccountBorrowFailed),
		);
		assert_eq!(account.try_borrow().unwrap()[0], 7);
		write_alias(&mut alias).unwrap();
		assert_eq!(account.try_borrow().unwrap()[0], 42);
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

[dependencies]
solana-account-view = "2"
solana-program-error = "3"

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
fn cargo_fix_preserves_named_guard_locks() {
	let scratch = ScratchCrate::new("unused-guards", UNFIXED_SOURCE);

	scratch.cargo(&["test", "--lib", "--offline", "--", "--nocapture"]);

	scratch.cargo(&[
		"fix",
		"--lib",
		"--offline",
		"--allow-dirty",
		"--allow-staged",
		"--allow-no-vcs",
	]);
	scratch.cargo(&["test", "--lib", "--offline", "--", "--nocapture"]);

	assert_eq!(
		scratch.source(),
		UNFIXED_SOURCE,
		"cargo fix must not shorten a named guard's lifetime"
	);

	assert_eq!(
		scratch.lint_warnings(),
		5,
		"all unread guard warnings should remain for manual review"
	);
}
