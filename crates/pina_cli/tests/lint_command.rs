#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use fs2::FileExt;
use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
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

fn hex_digest(bytes: &[u8]) -> String {
	const HEX: &[u8; 16] = b"0123456789abcdef";
	let mut encoded = String::with_capacity(bytes.len() * 2);
	for byte in bytes {
		encoded.push(char::from(HEX[usize::from(byte >> 4)]));
		encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
	}
	encoded
}

fn dylint_library_filename(name: &str, toolchain: &str) -> String {
	if cfg!(target_os = "macos") {
		format!("lib{name}@{toolchain}.dylib")
	} else {
		format!("lib{name}@{toolchain}.so")
	}
}

fn write_cached_bundle(cargo_home: &Path) {
	let catalog: Value = serde_json::from_str(include_str!("../lints.json"))
		.unwrap_or_else(|error| panic!("failed to parse lint catalog: {error}"));
	let target = env!("PINA_BUILD_TARGET");
	let toolchain = format!(
		"{}-{target}",
		catalog["toolchain"]
			.as_str()
			.unwrap_or_else(|| panic!("catalog toolchain should be a string"))
	);
	let bundle = cargo_home
		.join("pina/lints")
		.join(format!("v{}", env!("CARGO_PKG_VERSION")))
		.join(target)
		.join("bundle");
	fs::create_dir_all(&bundle)
		.unwrap_or_else(|error| panic!("failed to create cached bundle: {error}"));
	let libraries = catalog["libraries"]
		.as_array()
		.unwrap_or_else(|| panic!("catalog libraries should be an array"))
		.iter()
		.map(|name| {
			let name = name
				.as_str()
				.unwrap_or_else(|| panic!("catalog library should be a string"));
			let file = dylint_library_filename(name, &toolchain);
			let contents = format!("test dylint library: {name}");
			fs::write(bundle.join(&file), contents.as_bytes())
				.unwrap_or_else(|error| panic!("failed to write cached lint: {error}"));
			json!({
				"name": name,
				"file": file,
				"sha256": hex_digest(Sha256::digest(contents.as_bytes()).as_ref()),
				"size": contents.len(),
			})
		})
		.collect::<Vec<_>>();
	let manifest = json!({
		"schema_version": 1,
		"version": env!("CARGO_PKG_VERSION"),
		"target": target,
		"toolchain": toolchain,
		"dylint_version": "6.0.4",
		"libraries": libraries,
	});
	fs::write(
		bundle.join("manifest.json"),
		serde_json::to_vec_pretty(&manifest)
			.unwrap_or_else(|error| panic!("failed to encode manifest: {error}")),
	)
	.unwrap_or_else(|error| panic!("failed to write manifest: {error}"));
}

fn write_cached_tools(cargo_home: &Path, cargo_dylint: &Path) {
	let target = env!("PINA_BUILD_TARGET");
	let bundle = cargo_home
		.join("pina/tools/dylint-v6.0.4")
		.join(target)
		.join("bundle");
	fs::create_dir_all(&bundle)
		.unwrap_or_else(|error| panic!("failed to create cached Dylint tools: {error}"));
	let executables = [
		("cargo-dylint", cargo_dylint.to_path_buf()),
		("dylint-link", bundle.join("dylint-link")),
	]
	.into_iter()
	.map(|(name, source)| {
		let file = name.to_owned();
		if name == "cargo-dylint" {
			fs::copy(&source, bundle.join(&file))
				.unwrap_or_else(|error| panic!("failed to cache cargo-dylint: {error}"));
		} else {
			executable(&source, "#!/usr/bin/env bash\nexit 0\n");
		}
		let bytes = fs::read(bundle.join(&file))
			.unwrap_or_else(|error| panic!("failed to read cached Dylint tool: {error}"));
		json!({
			"name": name,
			"file": file,
			"sha256": hex_digest(Sha256::digest(&bytes).as_ref()),
			"size": bytes.len(),
		})
	})
	.collect::<Vec<_>>();
	let manifest = json!({
		"schema_version": 1,
		"dylint_version": "6.0.4",
		"target": target,
		"executables": executables,
	});
	fs::write(
		bundle.join("manifest.json"),
		serde_json::to_vec_pretty(&manifest)
			.unwrap_or_else(|error| panic!("failed to encode Dylint tool manifest: {error}")),
	)
	.unwrap_or_else(|error| panic!("failed to write Dylint tool manifest: {error}"));
}

struct Fixture {
	_temp: TempDir,
	project: PathBuf,
	cargo_home: PathBuf,
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
		write_cached_bundle(&cargo_home);
		let cargo_dylint = temp.path().join("cargo-dylint");
		let log = temp.path().join("commands.log");
		executable(
			&cargo_dylint,
			r#"#!/usr/bin/env bash
set -euo pipefail
printf 'dylint' >> "$PINA_LINT_LOG"
printf ' %q' "$@" >> "$PINA_LINT_LOG"
printf '\n' >> "$PINA_LINT_LOG"
if [[ "${FAKE_LINT_FAIL:-0}" == "1" ]]; then
	exit 43
fi
"#,
		);
		write_cached_tools(&cargo_home, &cargo_dylint);

		Self {
			_temp: temp,
			project,
			cargo_home,
			log,
		}
	}

	fn command(&self) -> Command {
		let real_cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
		let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
		command
			.args(["lint", "--project"])
			.arg(&self.project)
			.env("CARGO_HOME", &self.cargo_home)
			.env("CARGO", real_cargo)
			.env("PINA_LINT_LOG", &self.log);
		command
	}

	fn log(&self) -> String {
		fs::read_to_string(&self.log)
			.unwrap_or_else(|error| panic!("failed to read command log: {error}"))
	}

	fn managed_tool_root(&self) -> PathBuf {
		self.cargo_home
			.join("pina/tools/dylint-v6.0.4")
			.join(env!("PINA_BUILD_TARGET"))
	}
}

#[test]
fn lint_reuses_precompiled_runner_and_release_lints() {
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
	assert!(first_log.contains("dylint dylint --no-deps --no-metadata"));
	assert!(first_log.contains("--lib-path"));
	assert!(first_log.contains("require_owner_before_token_cast"));
	assert!(first_log.contains("--package lint-fixture"));
	assert!(!first_log.contains("--git"));
	assert!(!first_log.contains("--rev"));
	assert!(!first_log.contains("install"));

	let second = fixture
		.command()
		.output()
		.unwrap_or_else(|error| panic!("failed to rerun pina lint: {error}"));
	assert!(second.status.success());
	let second_log = fixture.log();
	assert_eq!(second_log.matches("dylint dylint").count(), 2);
}

#[test]
fn lint_fix_forwards_machine_applicable_fix_permissions() {
	let fixture = Fixture::new("pina-lint-fix");
	let output = fixture
		.command()
		.arg("--fix")
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina lint --fix: {error}"));
	assert!(output.status.success());
	assert!(
		fixture
			.log()
			.contains("--fix -- --allow-dirty --allow-staged --allow-no-vcs")
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
			.contains("Could not create the managed lint bundle directory")
	);
}

#[test]
fn lint_reports_a_concurrent_managed_runner_download() {
	let fixture = Fixture::new("pina-lint-lock-contention");
	let lock_path = fixture.managed_tool_root().join("bundle.lock");
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
		String::from_utf8_lossy(&output.stderr).contains("Could not lock the managed lint bundle")
	);
}
