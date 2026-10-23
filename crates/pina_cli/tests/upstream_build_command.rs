#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// A shell script that pretends to be an executable (it exits 0 for any
/// invocation, including `--version`).
fn fake_executable(dir: &Path, name: &str, body: &str) -> PathBuf {
	let path = dir.join(name);
	fs::write(&path, format!("#!/usr/bin/env bash\n{body}\n"))
		.unwrap_or_else(|error| panic!("failed to write {name}: {error}"));
	let mut permissions = fs::metadata(&path)
		.unwrap_or_else(|error| panic!("failed to inspect {name}: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&path, permissions)
		.unwrap_or_else(|error| panic!("failed to make {name} executable: {error}"));
	path
}

/// A fake cargo that answers `metadata` with the real cargo and records each
/// upstream build invocation's argv plus the driver-set environment, then
/// fabricates the cdylib the driver expects at
/// `target/bpfel-unknown-none/release/lib<name>.so`.
///
/// With `FAKE_UPSTREAM_MISSING_EXPORT` set, the first build invocation fails
/// with sbpf-linker's missing-libcall diagnostic so the retry path can be
/// observed; the sentinel file marks the diagnostic as delivered.
fn fake_upstream_cargo(dir: &Path) -> PathBuf {
	fake_executable(
		dir,
		"fake-cargo.sh",
		r#"
if [[ "${1:-}" == "metadata" ]]; then
	exec "${REAL_CARGO:-cargo}" "$@"
fi

{
	printf 'argv:'
	printf ' %q' "$@"
	printf '\n'
	printf 'rustflags=%s\n' "${CARGO_TARGET_BPFEL_UNKNOWN_NONE_RUSTFLAGS:-}"
	printf 'host_strip=%s\n' "${CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP-unset}"
	printf 'lto=%s\n' "${CARGO_PROFILE_RELEASE_LTO:-}"
} >> "$FAKE_UPSTREAM_ARGS"

if [[ "${FAKE_UPSTREAM_MISSING_EXPORT:-0}" == "1" ]] && [[ ! -f "$FAKE_UPSTREAM_MISSING_EXPORT_SENTINEL" ]]; then
	touch "$FAKE_UPSTREAM_MISSING_EXPORT_SENTINEL"
	printf 'error: linking with `sbpf-linker` failed: exit status: 1\n' >&2
	printf '  = note: Error: Symbol __udivti3 is missing. Export it with the `--export=__udivti3` flag.\n' >&2
	exit 1
fi

mkdir -p "$CARGO_TARGET_DIR/bpfel-unknown-none/release"
printf 'upstream-elf' > "$CARGO_TARGET_DIR/bpfel-unknown-none/release/libmy_program.so"
"#,
	)
}

/// Write a dependency-free pina project, so `cargo metadata` resolves offline
/// in every environment. The program source exercises the pina macros the
/// IDL generator parses.
fn write_project(root: &Path) {
	fs::create_dir_all(root.join("src"))
		.unwrap_or_else(|error| panic!("failed to create source directory: {error}"));
	fs::write(
		root.join("Cargo.toml"),
		r#"[package]
name = "my_program"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[features]
bpf-entrypoint = []
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write Cargo.toml: {error}"));
	fs::write(
		root.join("pina.toml"),
		r#"[project]
program = "."
# Routes `pina build` to the upstream sbpf-linker toolchain.
sbpf-linker = true
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write pina.toml: {error}"));
	fs::write(
		root.join("src/lib.rs"),
		r#"use pina::*;

declare_id!("11111111111111111111111111111111");

#[discriminator]
pub enum UpstreamInstruction {
	Hello = 0,
}

#[instruction(discriminator = UpstreamInstruction::Hello)]
pub struct HelloInstruction {}

#[derive(Accounts)]
pub struct HelloAccounts<'a> {
	pub user: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for HelloAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _ = HelloInstruction::try_from_bytes(data)?;
		self.user.assert_signer()?;
		Ok(())
	}
}

pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: UpstreamInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		UpstreamInstruction::Hello => {
			HelloAccounts::try_from((program_id, accounts))?.process(data)
		}
	}
}
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write lib.rs: {error}"));
}

/// Real cargo, for the fake's `metadata` passthrough.
fn real_cargo() -> String {
	std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

#[test]
fn upstream_build_drives_the_upstream_recipe() {
	let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let project = dir.path().join("project");
	write_project(&project);
	let fake_dir = dir.path().join("fake");
	fs::create_dir_all(&fake_dir).unwrap_or_else(|error| panic!("fake dir: {error}"));
	let cargo = fake_upstream_cargo(&fake_dir);
	let linker = fake_executable(&fake_dir, "fake-sbpf-linker.sh", "exit 0");
	let args_log = dir.path().join("invocations.log");
	let target_dir = dir.path().join("target");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("build")
		.arg("--project")
		.arg(&project)
		.env("CARGO", &cargo)
		.env("REAL_CARGO", real_cargo())
		.env("PINA_SBPF_LINKER", &linker)
		.env("FAKE_UPSTREAM_ARGS", &args_log)
		.env("CARGO_TARGET_DIR", &target_dir)
		.env_remove("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP")
		.output()
		.unwrap_or_else(|error| panic!("failed to execute pina binary: {error}"));
	assert!(
		output.status.success(),
		"expected successful upstream build, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let log = fs::read_to_string(&args_log)
		.unwrap_or_else(|error| panic!("read invocation log: {error}"))
		// Undo bash's `%q` escaping, whose exact output differs between bash
		// versions (for example the comma in build-std).
		.replace('\\', "");
	let manifest = fs::canonicalize(project.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("canonicalize manifest: {error}"));
	assert!(
		log.contains(&format!(
			"argv: build --release --manifest-path {} --target bpfel-unknown-none --features \
			 bpf-entrypoint -Z build-std=core,alloc",
			manifest.to_string_lossy()
		)),
		"expected the upstream cargo invocation, log: {log}"
	);
	assert!(
		log.contains("rustflags=-C linker=sbpf-linker"),
		"expected the upstream rustflags, log: {log}"
	);
	assert!(
		log.contains("--cfg=target_os=\"solana\"")
			&& log.contains("--cfg=target_feature=\"static-syscalls\""),
		"expected the static-syscall cfg flags, log: {log}"
	);
	// A project manifest may declare `lto = "fat"` (`pina init` scaffolds
	// one); the upstream build must override it, because fat LTO inflates
	// runtime compute units and blocks u128 division under sbpf-linker.
	assert!(
		log.contains("lto=false"),
		"expected LTO to be forced off, log: {log}"
	);
	// Darwin 27's dyld rejects proc macros stripped by an llvm-objcopy older
	// than LLVM 23, which an upstream nightly may still bundle.
	assert!(
		log.contains("host_strip=false"),
		"expected host units to stay unstripped, log: {log}"
	);

	assert!(
		target_dir.join("deploy/my_program.so").is_file(),
		"expected the upstream artifact to be published to target/deploy"
	);
	assert!(
		target_dir.join("idl/my_program.json").is_file(),
		"expected the IDL next to the deployed artifact"
	);
}

#[test]
fn upstream_build_retries_with_the_linker_s_missing_export() {
	let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let project = dir.path().join("project");
	write_project(&project);
	let fake_dir = dir.path().join("fake");
	fs::create_dir_all(&fake_dir).unwrap_or_else(|error| panic!("fake dir: {error}"));
	let cargo = fake_upstream_cargo(&fake_dir);
	let linker = fake_executable(&fake_dir, "fake-sbpf-linker.sh", "exit 0");
	let args_log = dir.path().join("invocations.log");
	let sentinel = dir.path().join("missing-export.sentinel");
	let target_dir = dir.path().join("target");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("build")
		.arg("--project")
		.arg(&project)
		.env("CARGO", &cargo)
		.env("REAL_CARGO", real_cargo())
		.env("PINA_SBPF_LINKER", &linker)
		.env("FAKE_UPSTREAM_ARGS", &args_log)
		.env("FAKE_UPSTREAM_MISSING_EXPORT", "1")
		.env("FAKE_UPSTREAM_MISSING_EXPORT_SENTINEL", &sentinel)
		.env("CARGO_TARGET_DIR", &target_dir)
		.output()
		.unwrap_or_else(|error| panic!("failed to execute pina binary: {error}"));
	assert!(
		output.status.success(),
		"expected the retry to succeed, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let log = fs::read_to_string(&args_log)
		.unwrap_or_else(|error| panic!("read invocation log: {error}"));
	assert_eq!(
		log.matches("argv: build --release").count(),
		2,
		"expected exactly two cargo invocations, log: {log}"
	);
	let (_, second) = log
		.split_once("lto=false\n")
		.expect("expected a second invocation after the first");
	assert!(
		second.contains("-C link-arg=--export=__udivti3"),
		"expected the retry to export the reported symbol, log: {log}"
	);
	assert!(
		target_dir.join("deploy/my_program.so").is_file(),
		"expected the artifact after the retry"
	);
}

#[test]
fn upstream_build_requires_a_runnable_sbpf_linker() {
	let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let project = dir.path().join("project");
	write_project(&project);
	let missing_linker = dir.path().join("missing-sbpf-linker");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("build")
		.arg("--project")
		.arg(&project)
		.env("PINA_SBPF_LINKER", &missing_linker)
		.output()
		.unwrap_or_else(|error| panic!("failed to execute pina binary: {error}"));
	assert!(
		!output.status.success(),
		"expected the build to fail without sbpf-linker"
	);
	assert!(
		String::from_utf8_lossy(&output.stderr).contains("sbpf-linker"),
		"expected the error to name sbpf-linker, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);
}

/// End-to-end: scaffold with `pina init --sbpf-linker` and build with the real
/// nightly toolchain and a real sbpf-linker binary. Requires network access
/// for the scaffolded program's dependencies.
#[test]
#[ignore = "compiles with the real nightly toolchain and a real sbpf-linker binary"]
fn upstream_build_compiles_a_real_scaffolded_program() {
	let dir = TempDir::new().unwrap_or_else(|error| panic!("temp dir: {error}"));
	let project = dir.path().join("project");

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("init")
		.arg("my_program")
		.arg("--path")
		.arg(&project)
		.arg("--sbpf-linker")
		.output()
		.unwrap_or_else(|error| panic!("failed to execute pina binary: {error}"));
	assert!(
		output.status.success(),
		"expected successful init, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);

	let config = fs::read_to_string(project.join(".cargo/config.toml"))
		.unwrap_or_else(|error| panic!("read scaffolded config: {error}"));
	assert!(config.contains("linker=sbpf-linker"), "config: {config}");
	assert!(config.contains("static-syscalls"), "config: {config}");

	let pina_toml = fs::read_to_string(project.join("pina.toml"))
		.unwrap_or_else(|error| panic!("read scaffolded pina.toml: {error}"));
	assert!(
		pina_toml.contains("sbpf-linker = true"),
		"expected the scaffold to record the upstream toolchain: {pina_toml}"
	);

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.arg("build")
		.arg("--project")
		.arg(&project)
		.output()
		.unwrap_or_else(|error| panic!("failed to execute pina binary: {error}"));
	assert!(
		output.status.success(),
		"expected a real upstream build, stderr: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(project.join("target/deploy/my_program.so").is_file());
}
