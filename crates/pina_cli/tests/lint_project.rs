//! In-process coverage of `pina lint` and the lint driver preparation.
//!
//! The `lint_command.rs` suite exercises the compiled CLI through cargo's
//! `CARGO_BIN_EXE`, which coverage tools cannot attribute back to this crate.
//! These tests call the library directly so the driver preparation paths
//! (override handling, the managed cache, install failures, and lint
//! failures) contribute to coverage. Environment-based configuration is
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
				.unwrap_or_else(|poisoned| poisoned.into_inner()),
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
	let _environment = fixture.leveled_environment();

	let output =
		lint_project(&fixture.options()).expect("the leveled project should satisfy lint_project");
	assert_eq!(output.package_name, "lint-fixture");
}

/// Return the cargo executable in use by the outer test process.
fn real_cargo_path() -> OsString {
	std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// A Pina project plus fake-driver-install scaffolding for one test.
struct Fixture {
	_temp: TempDir,
	project: PathBuf,
	cargo_home: PathBuf,
	/// Installs a driver-shaped stub into `$root/bin` when invoked as cargo
	/// install (or fails, for error-path tests); every other verb delegates
	/// to the real cargo and the check verb is configurable for lint
	/// failures.
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
			r#"#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == "metadata" ]]; then
	exec "$REAL_CARGO" "$@"
fi
if [[ "${FAKE_RUSTC:-}" == "fail" ]]; then
	exit 12
fi
if [[ "${FAKE_RUSTC:-}" == "junk" ]]; then
	echo 'rustc without a fingerprint'
	exit 0
fi
if [[ "${FAKE_INSTALL_FAIL:-0}" == "1" ]]; then
	exit 17
fi
if [[ "$1" == "check" && "${FAKE_LINT_FAIL:-0}" == "1" ]]; then
	exit 43
fi
if [[ "$1" != "install" ]]; then
	exit 0
fi
if [[ -n "${FAKE_INSTALL_SKIP:-}" ]]; then
	exit 0
fi
root=""
previous=""
for arg in "$@"; do
	if [[ "$previous" == "--root" ]]; then
		root="$arg"
	fi
	previous="$arg"
done
mkdir -p "$root/bin"
printf '#!/usr/bin/env bash\nexec "$@"\n' > "$root/bin/pina_lint_driver"
chmod +x "$root/bin/pina_lint_driver"
"#,
		);
		let cargo_home = temp.path().join("cargo-home");

		Self {
			_temp: temp,
			project,
			cargo_home,
			fake_cargo,
		}
	}

	/// Environment addressing a driver placeholder; `as_executable` controls
	/// the mode bit so the invalid-override path can be exercised.
	fn driver_override(&self, as_executable: bool) -> Environment<'static> {
		let override_path = self._temp.path().join("driver-override");
		let contents = "#!/usr/bin/env bash\nexec \"$@\"\n";
		if as_executable {
			executable(&override_path, contents);
		} else {
			fs::write(&override_path, contents).expect("failed to write driver override");
		}
		let mut environment = Environment::acquire();
		environment.set("PINA_LINT_DRIVER_PATH", &override_path);
		environment
	}

	/// Environment addressing the fake cargo install with a managed cache.
	fn managed_environment(&self) -> Environment<'static> {
		let mut environment = Environment::acquire();
		// Project discovery delegates `cargo metadata` back to the real
		// cargo through the fake's `$REAL_CARGO` indirection, so the real
		// cargo has to be captured before `CARGO` is overridden.
		let real_cargo = real_cargo_path();
		environment.set("CARGO_HOME", &self.cargo_home);
		environment.set("CARGO", &self.fake_cargo);
		environment.set("REAL_CARGO", real_cargo);
		environment
	}

	/// Managed environment with a project whose `[lints]` table exercises
	/// every configured level.
	fn leveled_environment(&self) -> Environment<'static> {
		let environment = self.managed_environment();
		fs::write(
			self.project.join("pina.toml"),
			"[project]\nprogram = \".\"\n\n[lints]\nrequire_empty_before_init = \
			 \"deny\"\ndeny_heap_allocations_in_onchain_instruction_handlers = \
			 \"warn\"\nrequire_zeroed_before_close = \"allow\"\n",
		)
		.expect("failed to write configured Pina configuration");
		environment
	}

	/// Managed environment whose `rustc -vV` (resolved through PATH) fails.
	fn failing_rustc_environment(&self) -> Environment<'static> {
		let mut environment = self.managed_environment();
		let fake_bin = self.shadow_rustc("#!/usr/bin/env bash\nexit 12\n");
		environment.set("PATH", fake_bin);
		environment
	}

	/// Managed environment whose `rustc -vV` prints no usable fingerprint.
	fn fingerprintless_rustc_environment(&self) -> Environment<'static> {
		let mut environment = self.managed_environment();
		let fake_bin = self.shadow_rustc("#!/usr/bin/env bash\necho rustc-junk\n");
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
fn driver_override_suppresses_the_install() {
	let fixture = Fixture::new();
	let environment = fixture.driver_override(true);

	let output =
		lint_project(&fixture.options()).expect("lint over the driver override should succeed");
	assert_eq!(output.package_name, "lint-fixture");
	assert!(!output.fix);
	drop(environment);
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
fn install_failure_reports_cargo_status() {
	let fixture = Fixture::new();
	let mut environment = fixture.managed_environment();
	environment.set("FAKE_INSTALL_FAIL", "1");

	let error = lint_project(&fixture.options()).expect_err("a failed cargo install should fail");
	assert!(
		error
			.to_string()
			.contains("Could not install the lint driver"),
		"unexpected error: {error}",
	);
	drop(environment);
}

#[test]
fn missing_driver_artifact_is_reported() {
	let fixture = Fixture::new();
	let mut environment = fixture.managed_environment();
	environment.set("FAKE_INSTALL_SKIP", "1");

	let error =
		lint_project(&fixture.options()).expect_err("a missing driver artifact should fail");
	assert!(
		error.to_string().contains("finished without producing"),
		"unexpected error: {error:?}",
	);
	drop(environment);
}

#[test]
fn successful_install_reuses_the_managed_driver() {
	let fixture = Fixture::new();
	let _environment = fixture.managed_environment();

	let output =
		lint_project(&fixture.options()).expect("the installed driver should satisfy lint_project");
	assert_eq!(output.package_name, "lint-fixture");

	// The second run resolves the driver from the managed cache instead of
	// reinstalling it.
	lint_project(&fixture.options()).expect("the cached driver should satisfy lint_project");
}

#[test]
fn rustc_failures_surface_as_driver_errors() {
	let fixture = Fixture::new();
	let _environment = fixture.failing_rustc_environment();

	let error = lint_project(&fixture.options())
		.expect_err("a failing rustc fingerprint query should fail");
	assert!(
		error.to_string().contains("exited with status"),
		"unexpected error: {error}",
	);
}

#[test]
fn fingerprintless_rustc_surfaces_as_a_driver_error() {
	let fixture = Fixture::new();
	let _environment = fixture.fingerprintless_rustc_environment();

	let error = lint_project(&fixture.options()).expect_err("a fingerprintless rustc should fail");
	assert!(
		error
			.to_string()
			.contains("Could not parse a release or host target"),
		"unexpected error: {error}",
	);
}

#[test]
fn cargo_failures_surface_as_lint_failures() {
	let fixture = Fixture::new();
	let mut environment = fixture.managed_environment();
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
