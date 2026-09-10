//! In-process coverage of `pina lint` and the lint driver resolution.
//!
//! The `lint_command.rs` suite exercises the compiled CLI through cargo's
//! `CARGO_BIN_EXE`, which coverage tools cannot attribute back to this crate.
//! These tests call the library directly so the driver resolution paths
//! (override handling, the sysroot query, and lint failures) contribute to
//! coverage. In-process calls resolve no bundled driver — the running test
//! binary's directory holds no `pina_lint_driver` — so every test installs a
//! `PINA_LINT_DRIVER_PATH` override. Environment-based configuration is
//! swapped per call under a mutex; a RAII guard restores the previous values
//! on drop.

#![cfg(unix)]
// The library under test reads its configuration from the process
// environment, and `std::env::set_var`/`remove_var` are unsafe in edition
// 2024. This suite is the only place that swaps the environment in-process;
// every mutation is guarded by `ENV_LOCK` and restored on drop.
#![allow(unsafe_code)]

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::MutexGuard;

use pina_cli::lint::LintError;
use pina_cli::lint::LintOptions;
use pina_cli::lint::lint_project;
use tempfile::TempDir;

/// Tests here mutate the process environment, so they run one at a time.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Applies environment overrides and restores the previous values on drop.
struct Environment<'a> {
	_guard: MutexGuard<'a, ()>,
	previous: Vec<(&'static str, Option<OsString>)>,
}

impl Environment<'_> {
	fn acquire() -> Self {
		Self {
			_guard: ENV_LOCK
				.lock()
				.unwrap_or_else(std::sync::PoisonError::into_inner),
			previous: Vec::new(),
		}
	}

	/// Override an environment variable, remembering the previous value.
	fn set(&mut self, key: &'static str, value: impl Into<OsString>) {
		self.record(key);
		let value = value.into();
		// SAFETY: tests holding `ENV_LOCK` are serialized, so no other thread
		// observes the race-ridden process environment.
		unsafe {
			std::env::set_var(key, value);
		}
	}

	fn record(&mut self, key: &'static str) {
		if !self.previous.iter().any(|(recorded, _)| recorded == &key) {
			self.previous.push((key, std::env::var_os(key)));
		}
	}
}

impl Drop for Environment<'_> {
	fn drop(&mut self) {
		for (key, value) in self.previous.iter().rev() {
			// SAFETY: same serialization guarantee as `Environment::acquire`.
			unsafe {
				match value {
					Some(value) => std::env::set_var(key, value),
					None => std::env::remove_var(key),
				}
			}
		}
	}
}

fn executable(path: &Path, contents: &str) {
	fs::write(path, contents)
		.unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
	let mut permissions = fs::metadata(path)
		.unwrap_or_else(|error| panic!("failed to inspect {}: {error}", path.display()))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(path, permissions)
		.unwrap_or_else(|error| panic!("failed to make {} executable: {error}", path.display()));
}

/// The configured-levels table reaches the driver through the environment
/// and changes the effective severity of the named rules.
#[test]
fn lint_forwards_configured_levels() {
	let fixture = Fixture::new();
	let environment = fixture.lint_environment();
	fs::write(
		fixture.project.join("pina.toml"),
		"[project]\nprogram = \".\"\n\n[lints]\nrequire_empty_before_init = \
		 \"deny\"\ndeny_heap_allocations_in_onchain_instruction_handlers = \
		 \"warn\"\nrequire_zeroed_before_close = \"allow\"\n",
	)
	.expect("failed to write configured Pina configuration");

	let output =
		lint_project(&fixture.options()).expect("the leveled project should satisfy lint_project");
	assert_eq!(output.package_name, "lint-fixture");
	drop(environment);
}

/// Return the cargo executable in use by the outer test process.
fn real_cargo_path() -> OsString {
	// The devenv shell exports `CARGO` as an empty string; treat that like an
	// unset variable so the fake's delegation reaches a real cargo.
	std::env::var_os("CARGO")
		.filter(|cargo| !cargo.is_empty())
		.unwrap_or_else(|| OsString::from("cargo"))
}

/// A Pina project plus fake-cargo scaffolding for one test.
struct Fixture {
	_temp: TempDir,
	project: PathBuf,
	/// Log the fake cargo appends its observed environment to.
	log: PathBuf,
	/// Logs the arguments it received plus the lint-driver environment, and
	/// delegates only project discovery back to the real cargo; the check
	/// verb is configurable for lint failures.
	fake_cargo: PathBuf,
}

impl Fixture {
	fn new() -> Self {
		let temp = tempfile::Builder::new()
			.prefix("pina-lint-library")
			.tempdir()
			.unwrap_or_else(|error| panic!("failed to create fixture: {error}"));
		let project = temp.path().join("project");
		fs::create_dir_all(project.join("src")).expect("failed to create project source");
		fs::write(
			project.join("Cargo.toml"),
			r#"[package]
name = "lint-fixture"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[workspace]
"#,
		)
		.expect("failed to write project manifest");
		fs::write(project.join("src/lib.rs"), "pub fn fixture() {}\n")
			.expect("failed to write project source");
		fs::write(
			project.join("Cargo.lock"),
			"version = 4\n\n[[package]]\nname = \"lint-fixture\"\nversion = \"0.1.0\"\n",
		)
		.expect("failed to write project lockfile");
		fs::write(project.join("pina.toml"), "[project]\nprogram = \".\"\n")
			.expect("failed to write Pina configuration");

		let fake_cargo = temp.path().join("cargo");
		executable(
			&fake_cargo,
			r#"#!/bin/bash
set -euo pipefail
if [[ "$1" == "metadata" ]]; then
  exec "$REAL_CARGO" "$@"
fi
printf 'library verb=%s dyld=%s ldlib=%s\n' "$1" \
  "${DYLD_LIBRARY_PATH-unset}" \
  "${LD_LIBRARY_PATH-unset}" >> "$PINA_LINT_LOG"
if [[ "$1" == "check" && "${FAKE_LINT_FAIL:-0}" == "1" ]]; then
  exit 43
fi
"#,
		);
		let log = temp.path().join("commands.log");

		Self {
			_temp: temp,
			project,
			log,
			fake_cargo,
		}
	}

	/// Environment pointing the CLI at the fixture's fake cargo and a driver
	/// override stub.
	fn lint_environment(&self) -> Environment<'static> {
		let mut environment = Environment::acquire();
		// Project discovery delegates `cargo metadata` back to the real
		// cargo through the fake's `$REAL_CARGO` indirection, so the real
		// cargo has to be captured before `CARGO` is overridden.
		let real_cargo = real_cargo_path();
		environment.set("CARGO", &self.fake_cargo);
		environment.set("REAL_CARGO", real_cargo);
		environment.set("PINA_LINT_LOG", &self.log);
		let driver = self._temp.path().join("driver-override");
		executable(&driver, "#!/bin/bash\nexec \"$@\"\n");
		environment.set("PINA_LINT_DRIVER_PATH", &driver);
		environment
	}

	/// Environment addressing a driver placeholder; `as_executable` controls
	/// the mode bit so the invalid-override path can be exercised.
	fn driver_override(&self, as_executable: bool) -> Environment<'static> {
		let override_path = self._temp.path().join("driver-override");
		let contents = "#!/bin/bash\nexec \"$@\"\n";
		if as_executable {
			executable(&override_path, contents);
		} else {
			fs::write(&override_path, contents).expect("failed to write driver override");
		}
		let mut environment = Environment::acquire();
		// The real cargo must be captured before `CARGO` is overridden with
		// the fake; reading it afterwards would hand the fake back to itself
		// and recurse forever.
		let real_cargo = real_cargo_path();
		environment.set("CARGO", &self.fake_cargo);
		environment.set("REAL_CARGO", real_cargo);
		environment.set("PINA_LINT_LOG", &self.log);
		environment.set("PINA_LINT_DRIVER_PATH", &override_path);
		environment
	}

	/// Lint environment whose `rustc --print sysroot` (resolved through PATH)
	/// fails.
	fn failing_rustc_environment(&self) -> Environment<'static> {
		let mut environment = self.lint_environment();
		let fake_bin = self.shadow_rustc("#!/bin/bash\nexit 12\n");
		environment.set("PATH", fake_bin);
		environment
	}

	/// A directory shadowing `rustc` with the given script, joined in front
	/// of the current PATH.
	fn shadow_rustc(&self, contents: &str) -> PathBuf {
		let fake_bin = self._temp.path().join("shadow-bin");
		fs::create_dir_all(&fake_bin).expect("failed to create fake bin directory");
		executable(&fake_bin.join("rustc"), contents);
		let remaining = std::env::var_os("PATH").unwrap_or_default();
		let joined = std::env::join_paths(
			std::iter::once(fake_bin).chain(std::env::split_paths(&remaining)),
		)
		.expect("failed to join the shadow PATH");
		PathBuf::from(joined)
	}

	fn options(&self) -> LintOptions {
		LintOptions {
			project: self.project.clone(),
			fix: false,
		}
	}
}

#[test]
fn driver_override_runs_the_lint_with_a_local_driver() {
	let fixture = Fixture::new();
	let _environment = fixture.lint_environment();

	let output =
		lint_project(&fixture.options()).expect("lint over the driver override should succeed");
	assert_eq!(output.package_name, "lint-fixture");
	assert!(!output.fix);
}

#[test]
fn driver_override_requires_an_executable() {
	let fixture = Fixture::new();
	let environment = fixture.driver_override(false);

	let error =
		lint_project(&fixture.options()).expect_err("a non-executable override should fail");
	assert!(
		matches!(
			error,
			LintError::Driver(pina_cli::lint_driver::DriverError::InvalidDriverOverride { .. })
		),
		"unexpected error: {error:?}",
	);
	drop(environment);
}

#[test]
fn sysroot_failures_surface_as_driver_errors() {
	let fixture = Fixture::new();
	let _environment = fixture.failing_rustc_environment();

	let error =
		lint_project(&fixture.options()).expect_err("a failing rustc sysroot query should fail");
	assert!(
		error.to_string().contains("exited with status"),
		"unexpected error: {error}",
	);
}

#[test]
fn cargo_failures_surface_as_lint_failures() {
	let fixture = Fixture::new();
	let mut environment = fixture.lint_environment();
	environment.record("FAKE_LINT_FAIL");
	environment.set("FAKE_LINT_FAIL", "1");

	let error = lint_project(&fixture.options())
		.expect_err("a failing lint run should surface as a lint failure");
	assert!(
		matches!(error, LintError::LintFailed { .. }),
		"unexpected error: {error:?}",
	);
	drop(environment);
}

/// Return the sysroot the CLI resolves for the fixture project.
fn project_sysroot(project: &Path) -> PathBuf {
	let output = std::process::Command::new("rustc")
		.arg("--print")
		.arg("sysroot")
		.current_dir(project)
		.output()
		.expect("rustc --print sysroot should run");
	assert!(
		output.status.success(),
		"rustc --print sysroot failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

#[test]
fn lint_runs_with_the_toolchain_library_search_path() {
	let fixture = Fixture::new();
	let _environment = fixture.lint_environment();

	lint_project(&fixture.options()).expect("the lint run should succeed");

	let log = fs::read_to_string(&fixture.log)
		.unwrap_or_else(|error| panic!("failed to read command log: {error}"));
	let library = log
		.lines()
		.find(|line| line.starts_with("library verb=check "))
		.unwrap_or_else(|| panic!("missing library environment for the check run: {log}"));
	let sysroot = project_sysroot(&fixture.project);
	// The fake cargo is a shell script, so macOS strips `DYLD_LIBRARY_PATH`
	// at the kernel boundary; `LD_LIBRARY_PATH` is set alongside it and
	// survives every platform under test.
	let expected = format!("ldlib={}", sysroot.join("lib").display());
	assert!(
		library.contains(&expected),
		"the lint cargo run must inherit the sysroot library directory: {library}"
	);
}
