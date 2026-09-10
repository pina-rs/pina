#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Name of the fake driver executable installed by each fixture.
const FAKE_DRIVER: &str = "fake-pina-lint-driver";

/// The fake cargo implementation used by every test: it logs the arguments it
/// received plus the lint-driver environment, and can be asked to fail.
const FAKE_CARGO: &str = r#"#!/bin/bash
set -euo pipefail
# Project discovery and other non-lint commands delegate to the real cargo;
# only the lint run itself is simulated.
if [[ "${1:-}" == "metadata" ]]; then
	exec "$REAL_CARGO" "$@"
fi
printf 'cargo' >> "$PINA_LINT_LOG"
printf ' %q' "$@" >> "$PINA_LINT_LOG"
printf '\n' >> "$PINA_LINT_LOG"
printf 'rustc_wrapper=%s workspace_wrapper=%s driver_build=%s no_deps=%s levels=%s\n' \
  "${RUSTC_WRAPPER-unset}" \
  "$(basename "$RUSTC_WORKSPACE_WRAPPER")" \
  "${PINA_LINT_DRIVER_BUILD:-}" \
  "${PINA_LINT_NO_DEPS:-}" \
  "${PINA_LINT_LEVELS:-}" >> "$PINA_LINT_LOG"
printf 'library dyld=%s ldlib=%s\n' \
  "${DYLD_LIBRARY_PATH-unset}" \
  "${LD_LIBRARY_PATH-unset}" >> "$PINA_LINT_LOG"
if [[ "${FAKE_LINT_FAIL:-0}" == "1" ]]; then
	exit 43
fi
"#;

fn write_project(root: &Path, lints: Option<&str>) {
	fs::create_dir_all(root.join("src"))
		.unwrap_or_else(|error| panic!("failed to create project source: {error}"));
	fs::write(
		root.join("Cargo.toml"),
		r#"[package]
name = "lint-fixture"
version = "0.1.0"
edition = "2024"

[lib]
path = "src/lib.rs"

[workspace]
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write project manifest: {error}"));
	fs::write(root.join("src/lib.rs"), "pub fn fixture() {}\n")
		.unwrap_or_else(|error| panic!("failed to write project source: {error}"));
	let config = match lints {
		Some(lints) => format!("[project]\nprogram = \".\"\n\n[lints]\n{lints}"),
		None => "[project]\nprogram = \".\"\n".to_owned(),
	};
	fs::write(root.join("pina.toml"), config)
		.unwrap_or_else(|error| panic!("failed to write Pina configuration: {error}"));
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

struct Fixture {
	_temp: TempDir,
	project: PathBuf,
	log: PathBuf,
	fake_cargo: PathBuf,
	fake_driver: PathBuf,
}

impl Fixture {
	fn new(prefix: &str, lints: Option<&str>) -> Self {
		let temp = tempfile::Builder::new()
			.prefix(prefix)
			.tempdir()
			.unwrap_or_else(|error| panic!("failed to create fixture: {error}"));
		let project = temp.path().join("project");
		fs::create_dir_all(&project)
			.unwrap_or_else(|error| panic!("failed to create project: {error}"));
		write_project(&project, lints);

		let log = temp.path().join("commands.log");
		let fake_cargo = temp.path().join("cargo");
		executable(&fake_cargo, FAKE_CARGO);
		let fake_driver = temp.path().join(FAKE_DRIVER);
		executable(&fake_driver, "#!/bin/bash\nexit 0\n");

		Self {
			_temp: temp,
			project,
			log,
			fake_cargo,
			fake_driver,
		}
	}

	fn command(&self) -> Command {
		let real_cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.args(["lint", "--project"])
			.arg(&self.project)
			.env_remove("RUSTC_WRAPPER")
			.env("CARGO", &self.fake_cargo)
			.env("REAL_CARGO", real_cargo)
			.env("PINA_LINT_DRIVER_PATH", &self.fake_driver)
			.env("PINA_LINT_LOG", &self.log);
		command
	}

	fn log(&self) -> String {
		fs::read_to_string(&self.log)
			.unwrap_or_else(|error| panic!("failed to read command log: {error}"))
	}
}

#[test]
fn lint_runs_cargo_check_with_the_configured_driver() {
	let fixture = Fixture::new("pina-lint-success", None);
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		String::from_utf8_lossy(&output.stdout)
			.contains("Pina security lints passed for lint-fixture")
	);

	let log = fixture.log();
	assert!(log.contains("cargo check --locked"), "log: {log}");
	assert!(log.contains("--package lint-fixture"), "log: {log}");
	assert!(log.contains("--manifest-path"), "log: {log}");
	let library = log
		.lines()
		.find(|line| line.starts_with("library dyld="))
		.unwrap_or_else(|| panic!("missing library environment in log: {log}"));
	assert!(
		library
			.split_whitespace()
			.any(|entry| entry.starts_with("ldlib=") && entry.ends_with("/lib")),
		"the lint run must export the toolchain sysroot library directory: {library}",
	);
	let environment = log
		.lines()
		.find(|line| line.starts_with("rustc_wrapper="))
		.unwrap_or_else(|| panic!("missing lint environment in log: {log}"));
	let identity = environment
		.strip_prefix(&format!(
			"rustc_wrapper=unset workspace_wrapper={FAKE_DRIVER} driver_build="
		))
		.and_then(|rest| rest.strip_suffix(" no_deps=1 levels="))
		.unwrap_or_else(|| panic!("unexpected lint environment: {environment}"));
	assert_eq!(identity.len(), 64, "driver build identity: {identity}");
	assert!(
		identity
			.chars()
			.all(|character| character.is_ascii_hexdigit()),
		"driver build identity: {identity}"
	);
}

#[test]
fn lint_preserves_an_inherited_rustc_wrapper() {
	let fixture = Fixture::new("pina-lint-outer-wrapper", None);
	let output = fixture
		.command()
		.env("RUSTC_WRAPPER", "outer-wrapper")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let log = fixture.log();
	let environment = log
		.lines()
		.find(|line| line.starts_with("rustc_wrapper="))
		.unwrap_or_else(|| panic!("missing lint environment in log: {log}"));
	assert!(
		environment.starts_with("rustc_wrapper=outer-wrapper workspace_wrapper="),
		"environment: {environment}"
	);
}

#[test]
fn lint_fix_uses_cargo_fix_with_edit_permissions() {
	let fixture = Fixture::new("pina-lint-fix", None);
	let output = fixture
		.command()
		.arg("--fix")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint --fix: {error}"));
	assert!(
		output.status.success(),
		"pina lint --fix failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let log = fixture.log();
	assert!(
		log.contains("cargo fix --allow-dirty --allow-staged --allow-no-vcs"),
		"log: {log}"
	);
}

#[test]
fn lint_forwards_configured_levels_to_the_driver() {
	let fixture = Fixture::new(
		"pina-lint-levels",
		Some(
			"require_empty_before_init = \
			 \"deny\"\ndeny_heap_allocations_in_onchain_instruction_handlers = \"warn\"\n",
		),
	);
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let log = fixture.log();
	assert!(
		log.contains(
			"levels=deny_heap_allocations_in_onchain_instruction_handlers=warn,\
			 require_empty_before_init=deny"
		),
		"log: {log}"
	);
}

#[test]
fn lint_rejects_unknown_lint_names() {
	let fixture = Fixture::new("pina-lint-unknown", Some("not_a_pina_lint = \"deny\"\n"));
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("Unknown pina lint `not_a_pina_lint` in [lints]"),
		"stderr: {stderr}"
	);
	assert!(
		stderr.contains("require_empty_before_init"),
		"stderr: {stderr}"
	);
}

#[test]
fn lint_rejects_invalid_lint_levels() {
	let fixture = Fixture::new(
		"pina-lint-level",
		Some("require_empty_before_init = \"error\"\n"),
	);
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("Invalid level `error` for lint `require_empty_before_init` in [lints]"),
		"stderr: {stderr}"
	);
}

#[test]
fn lint_propagates_cargo_failures() {
	let fixture = Fixture::new("pina-lint-cargo-failure", None);
	let output = fixture
		.command()
		.env("FAKE_LINT_FAIL", "1")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("security lints failed with status"),
		"stderr: {stderr}"
	);
}

#[test]
fn lint_rejects_a_non_executable_driver_override() {
	let fixture = Fixture::new("pina-lint-driver-override", None);
	let stale = fixture.project.join("stale-driver");
	fs::write(&stale, "not executable\n")
		.unwrap_or_else(|error| panic!("failed to write stale driver: {error}"));
	let output = fixture
		.command()
		.env("PINA_LINT_DRIVER_PATH", &stale)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("does not point at an executable"),
		"stderr: {stderr}"
	);
}

/// Coverage of the bundled-driver resolution: the CLI is copied to an
/// isolated directory so the lookup next to the executable can be exercised
/// with or without a driver present.
struct BundledFixture {
	_temp: TempDir,
	project: PathBuf,
	log: PathBuf,
	pina: PathBuf,
	driver: PathBuf,
	fake_cargo: PathBuf,
}

impl BundledFixture {
	/// `driver_contents` of `None` leaves the copy without a driver, and
	/// otherwise writes the given script at the bundled-driver path.
	fn new(prefix: &str, driver_contents: Option<&str>) -> Self {
		let temp = tempfile::Builder::new()
			.prefix(prefix)
			.tempdir()
			.unwrap_or_else(|error| panic!("failed to create fixture: {error}"));
		let project = temp.path().join("project");
		fs::create_dir_all(project.join("src"))
			.unwrap_or_else(|error| panic!("failed to create project: {error}"));
		write_project(&project, None);

		let log = temp.path().join("commands.log");
		let fake_cargo = temp.path().join("cargo");
		executable(&fake_cargo, FAKE_CARGO);

		let pina = temp.path().join("pina");
		fs::copy(env!("CARGO_BIN_EXE_pina"), &pina)
			.unwrap_or_else(|error| panic!("failed to copy the CLI: {error}"));

		let driver = temp.path().join("pina_lint_driver");
		if let Some(contents) = driver_contents {
			executable(&driver, contents);
		}

		Self {
			_temp: temp,
			project,
			log,
			pina,
			driver,
			fake_cargo,
		}
	}

	fn command(&self) -> Command {
		let real_cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
		let mut command = Command::new(&self.pina);
		command
			.args(["lint", "--project"])
			.arg(&self.project)
			.env_remove("RUSTC_WRAPPER")
			.env("CARGO", &self.fake_cargo)
			.env("REAL_CARGO", real_cargo)
			.env("PINA_LINT_LOG", &self.log);
		command
	}

	fn log(&self) -> String {
		fs::read_to_string(&self.log)
			.unwrap_or_else(|error| panic!("failed to read command log: {error}"))
	}
}

#[test]
fn lint_runs_the_prebuilt_driver_bundled_next_to_the_cli() {
	let fixture = BundledFixture::new("pina-lint-bundled", Some("#!/bin/bash\nexit 0\n"));
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		String::from_utf8_lossy(&output.stdout)
			.contains("Pina security lints passed for lint-fixture")
	);

	let log = fixture.log();
	assert!(
		log.contains("workspace_wrapper=pina_lint_driver"),
		"the bundled driver must be the workspace wrapper: {log}"
	);
	assert!(
		log.contains("driver_build="),
		"the bundled driver content identity must reach cargo: {log}"
	);
}

#[test]
fn unloadable_bundled_driver_reports_the_required_toolchain() {
	let fixture = BundledFixture::new("pina-lint-bundled-broken", Some("#!/bin/bash\nexit 9\n"));
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("Could not load the lint driver"),
		"stderr: {stderr}"
	);
	assert!(
		stderr.contains("nightly-2026-02-20") && stderr.contains("rust-toolchain.toml"),
		"the error should name the required toolchain and the pin file: {stderr}"
	);
}

#[test]
fn missing_bundled_driver_is_reported() {
	let fixture = BundledFixture::new("pina-lint-bundled-missing", None);
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("Could not find the prebuilt lint driver"),
		"stderr: {stderr}"
	);
}
