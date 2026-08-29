#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use fs2::FileExt;
use tempfile::TempDir;

fn write_project(root: &Path) {
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
	fs::write(root.join("Pina.toml"), "[project]\nprogram = \".\"\n")
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
	cargo_home: PathBuf,
	cargo: PathBuf,
	cargo_dylint: PathBuf,
	dylint_link: PathBuf,
	log: PathBuf,
}

impl Fixture {
	fn new(prefix: &str) -> Self {
		let temp = tempfile::Builder::new()
			.prefix(prefix)
			.tempdir()
			.unwrap_or_else(|error| panic!("failed to create fixture: {error}"));
		let project = temp.path().join("project");
		fs::create_dir_all(&project)
			.unwrap_or_else(|error| panic!("failed to create project: {error}"));
		write_project(&project);

		let cargo_home = temp.path().join("cargo-home");
		let cargo = temp.path().join("cargo");
		let cargo_dylint = temp.path().join("cargo-dylint");
		let dylint_link = temp.path().join("dylint-link");
		let log = temp.path().join("commands.log");
		executable(
			&cargo,
			r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "metadata" ]]; then
	"$REAL_CARGO" "$@"
	status="$?"
	if [[ "${FAKE_CARGO_DISAPPEAR:-0}" == "1" ]]; then
		rm "$0"
	fi
	exit "$status"
fi

printf 'cargo' >> "$PINA_LINT_LOG"
printf ' %q' "$@" >> "$PINA_LINT_LOG"
printf '\n' >> "$PINA_LINT_LOG"

package="${!#}"
if [[ "${FAKE_INSTALL_FAIL:-}" == "$package" ]]; then
	exit 41
fi

root=""
previous=""
for argument in "$@"; do
	if [[ "$previous" == "--root" ]]; then
		root="$argument"
		break
	fi
	previous="$argument"
done
test -n "$root"
mkdir -p "$root/bin"

if [[ "${FAKE_INSTALL_OMIT:-}" == "$package" ]]; then
	exit 0
fi

case "$package" in
	cargo-dylint) cp "$FAKE_CARGO_DYLINT_SOURCE" "$root/bin/cargo-dylint" ;;
	dylint-link) cp "$FAKE_DYLINT_LINK_SOURCE" "$root/bin/dylint-link" ;;
	*) exit 42 ;;
esac
"#,
		);
		executable(
			&cargo_dylint,
			r#"#!/usr/bin/env bash
set -euo pipefail
printf 'dylint' >> "$PINA_LINT_LOG"
printf ' %q' "$@" >> "$PINA_LINT_LOG"
printf '\n' >> "$PINA_LINT_LOG"
printf 'link=%s\n' "$(command -v dylint-link)" >> "$PINA_LINT_LOG"
if [[ "${FAKE_LINT_FAIL:-0}" == "1" ]]; then
	exit 43
fi
"#,
		);
		executable(&dylint_link, "#!/usr/bin/env bash\nexit 0\n");

		Self {
			_temp: temp,
			project,
			cargo_home,
			cargo,
			cargo_dylint,
			dylint_link,
			log,
		}
	}

	fn command(&self) -> Command {
		let real_cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.args(["lint", "--project"])
			.arg(&self.project)
			.env("CARGO", &self.cargo)
			.env("CARGO_HOME", &self.cargo_home)
			.env("REAL_CARGO", real_cargo)
			.env("PINA_LINT_LOG", &self.log)
			.env("FAKE_CARGO_DYLINT_SOURCE", &self.cargo_dylint)
			.env("FAKE_DYLINT_LINK_SOURCE", &self.dylint_link);
		command
	}

	fn log(&self) -> String {
		fs::read_to_string(&self.log)
			.unwrap_or_else(|error| panic!("failed to read command log: {error}"))
	}

	fn managed_tool_root(&self) -> PathBuf {
		self.cargo_home.join("pina/tools/dylint-6.0.4-6.0.4")
	}
}

#[test]
fn lint_installs_pinned_tools_once_and_runs_release_lints() {
	let fixture = Fixture::new("pina-lint-success");
	let first = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		first.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&first.stderr)
	);
	assert!(
		String::from_utf8_lossy(&first.stdout)
			.contains("Pina security lints passed for lint-fixture")
	);

	let first_log = fixture.log();
	assert!(first_log.contains("install --locked --root"));
	assert!(first_log.contains("--version 6.0.4 cargo-dylint"));
	assert!(first_log.contains("--version 6.0.4 dylint-link"));
	assert!(first_log.contains("dylint dylint --no-deps"));
	assert!(first_log.contains("--git https://github.com/pina-rs/pina"));
	assert!(first_log.contains("--rev 7aff2afb89cd34a13e1e9d6ff854bc16d12263cc"));
	assert!(first_log.contains("--pattern lints/\\*"));
	assert!(first_log.contains("--package lint-fixture"));
	assert!(first_log.contains("link="));
	assert!(first_log.contains("/cargo-home/pina/tools/dylint-6.0.4-6.0.4/bin/dylint-link"));

	let second = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to rerun pina lint: {error}"));
	assert!(second.status.success());
	let second_log = fixture.log();
	assert_eq!(second_log.matches("cargo install").count(), 2);
	assert_eq!(second_log.matches("dylint dylint").count(), 2);
}

#[test]
fn lint_discovers_cargo_from_path_when_the_cargo_variable_is_absent() {
	let fixture = Fixture::new("pina-lint-cargo-path");
	let fake_bin = fixture
		.cargo
		.parent()
		.unwrap_or_else(|| panic!("fake Cargo should have a parent directory"));
	let current_path = std::env::var_os("PATH").unwrap_or_default();
	let path = std::iter::once(fake_bin.to_path_buf()).chain(std::env::split_paths(&current_path));
	let path = std::env::join_paths(path)
		.unwrap_or_else(|error| panic!("failed to construct test PATH: {error}"));
	let output = fixture
		.command()
		.env_remove("CARGO")
		.env("PATH", path)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(fixture.log().contains("cargo install --locked --root"));
}

#[test]
fn lint_resolves_relative_cargo_home_from_the_invocation_directory() {
	let fixture = Fixture::new("pina-lint-relative-cargo-home");
	let invocation_directory = fixture._temp.path().join("invocation");
	fs::create_dir_all(&invocation_directory)
		.unwrap_or_else(|error| panic!("failed to create invocation directory: {error}"));
	let output = fixture
		.command()
		.current_dir(&invocation_directory)
		.env("CARGO_HOME", "relative-cargo-home")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		invocation_directory
			.join("relative-cargo-home/pina/tools/dylint-6.0.4-6.0.4/bin/cargo-dylint")
			.is_file()
	);
}

#[test]
fn lint_defaults_managed_tools_to_the_platform_cargo_home() {
	let fixture = Fixture::new("pina-lint-default-cargo-home");
	let home = fixture._temp.path().join("home");
	fs::create_dir_all(&home)
		.unwrap_or_else(|error| panic!("failed to create platform home: {error}"));
	let output = fixture
		.command()
		.env_remove("CARGO_HOME")
		.env("HOME", &home)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(
		output.status.success(),
		"pina lint failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		home.join(".cargo/pina/tools/dylint-6.0.4-6.0.4/bin/cargo-dylint")
			.is_file()
	);
}

#[test]
fn lint_reports_project_discovery_failures() {
	let fixture = Fixture::new("pina-lint-discovery-failure");
	let missing = fixture.project.join("missing");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["lint", "--project"])
		.arg(&missing)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("Could not inspect"));
}

#[test]
fn lint_fix_allows_cargo_to_edit_dirty_and_staged_sources() {
	let fixture = Fixture::new("pina-lint-fix");
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
	assert!(
		String::from_utf8_lossy(&output.stdout)
			.contains("Applied available Pina security lint fixes for lint-fixture")
	);
	assert!(
		fixture.log().contains(
			"--package lint-fixture --fix -- --allow-dirty --allow-staged --allow-no-vcs"
		)
	);
}

#[test]
fn lint_reports_pinned_tool_install_failure() {
	let fixture = Fixture::new("pina-lint-install-failure");
	let output = fixture
		.command()
		.env("FAKE_INSTALL_FAIL", "cargo-dylint")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Installing pinned tool `cargo-dylint` failed with status")
	);
}

#[test]
fn lint_rejects_a_successful_install_without_the_expected_binary() {
	let fixture = Fixture::new("pina-lint-missing-tool");
	let output = fixture
		.command()
		.env("FAKE_INSTALL_OMIT", "dylint-link")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Cargo reported success but did not install `dylint-link`")
	);
}

#[test]
fn lint_propagates_lint_failures() {
	let fixture = Fixture::new("pina-lint-lint-failure");
	let output = fixture
		.command()
		.env("FAKE_LINT_FAIL", "1")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("security lints failed with status"));
}

#[test]
fn lint_reports_when_cargo_disappears_after_discovery() {
	let fixture = Fixture::new("pina-lint-cargo-disappears");
	let output = fixture
		.command()
		.env("FAKE_CARGO_DISAPPEAR", "1")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Could not install pinned tool `cargo-dylint` with Cargo")
	);
}

#[test]
fn lint_reports_when_the_cached_runner_is_not_executable() {
	let fixture = Fixture::new("pina-lint-runner-permissions");
	let mut permissions = fs::metadata(&fixture.cargo_dylint)
		.unwrap_or_else(|error| panic!("failed to inspect fake cargo-dylint: {error}"))
		.permissions();
	permissions.set_mode(0o644);
	fs::set_permissions(&fixture.cargo_dylint, permissions)
		.unwrap_or_else(|error| panic!("failed to remove execute permission: {error}"));
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("Could not run Pina"),
		"unexpected stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

#[test]
fn lint_reports_an_invalid_managed_tool_path() {
	let fixture = Fixture::new("pina:lint-invalid-path");
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Could not construct PATH for the pinned Dylint tools")
	);
}

#[test]
fn lint_reports_when_the_managed_tool_directory_cannot_be_created() {
	let fixture = Fixture::new("pina-lint-tool-directory");
	let cargo_home = fixture.project.join("blocked-cargo-home");
	fs::write(&cargo_home, "not a directory")
		.unwrap_or_else(|error| panic!("failed to block Cargo home: {error}"));
	let output = fixture
		.command()
		.env("CARGO_HOME", &cargo_home)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Could not create the managed Dylint directory")
	);
}

#[test]
fn lint_reports_when_the_install_lock_cannot_be_opened() {
	let fixture = Fixture::new("pina-lint-open-lock");
	let lock = fixture.managed_tool_root().join("install.lock");
	fs::create_dir_all(&lock)
		.unwrap_or_else(|error| panic!("failed to create blocking lock directory: {error}"));
	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Could not open the managed Dylint installation lock")
	);
}

#[test]
fn lint_reports_a_concurrent_managed_tool_installation() {
	let fixture = Fixture::new("pina-lint-lock-contention");
	let lock_path = fixture.managed_tool_root().join("install.lock");
	fs::create_dir_all(
		lock_path
			.parent()
			.unwrap_or_else(|| panic!("lock should have a parent")),
	)
	.unwrap_or_else(|error| panic!("failed to create tool root: {error}"));
	let lock = fs::File::create(&lock_path)
		.unwrap_or_else(|error| panic!("failed to create install lock: {error}"));
	lock.lock_exclusive()
		.unwrap_or_else(|error| panic!("failed to acquire install lock: {error}"));

	let output = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint: {error}"));
	assert!(!output.status.success());
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Could not lock the managed Dylint installation")
	);
}
