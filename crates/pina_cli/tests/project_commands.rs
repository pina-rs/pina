#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

fn write_project(root: &Path) {
	fs::create_dir_all(root.join("src"))
		.unwrap_or_else(|error| panic!("failed to create source directory: {error}"));
	fs::write(
		root.join("Cargo.toml"),
		r#"[package]
name = "hyphen-package"
version = "0.1.0"
edition = "2024"

[lib]
name = "custom_program"
crate-type = ["cdylib", "lib"]

[features]
default = []
bpf-entrypoint = []
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write Cargo.toml: {error}"));
	fs::write(
		root.join("Pina.toml"),
		r#"[project]
program = "."

[clients]
output = "clients"
languages = ["rust", "typescript"]
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write Pina.toml: {error}"));
	fs::write(
		root.join("src/lib.rs"),
		r#"use pina::*;

declare_id!("11111111111111111111111111111111");

#[discriminator]
pub enum CustomInstruction {
	Initialize = 0,
}

#[instruction(discriminator = CustomInstruction::Initialize)]
pub struct InitializeInstruction {
	pub value: u8,
}

#[derive(Accounts)]
pub struct InitializeAccounts<'a> {
	pub payer: &'a AccountView,
}

impl<'a> ProcessAccountInfos<'a> for InitializeAccounts<'a> {
	fn process(self, data: &[u8]) -> ProgramResult {
		let _args = InitializeInstruction::try_from_bytes(data)?;
		self.payer.assert_signer()?;
		Ok(())
	}
}

pub fn process_instruction(
	program_id: &Address,
	accounts: &mut [AccountView],
	data: &[u8],
) -> ProgramResult {
	let instruction: CustomInstruction = parse_instruction(program_id, &ID, data)?;

	match instruction {
		CustomInstruction::Initialize => {
			InitializeAccounts::try_from((program_id, accounts))?.process(data)
		}
	}
}
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write lib.rs: {error}"));
}

fn fake_cargo(root: &Path) -> PathBuf {
	let path = root.join("fake-cargo.sh");
	fs::write(
		&path,
		r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "metadata" ]]; then
	exec "$REAL_CARGO" "$@"
fi

if [[ "${FAKE_CARGO_FAIL:-0}" == "1" ]]; then
	exit 23
fi

if [[ -n "${FAKE_CARGO_ARGS:-}" ]]; then
	printf '%s\n' "$@" > "$FAKE_CARGO_ARGS"
fi

mkdir -p "$CARGO_TARGET_DIR/bpfel-unknown-none/release"
printf 'compiled-sbf' > "$CARGO_TARGET_DIR/bpfel-unknown-none/release/libcustom_program.so"
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write fake cargo: {error}"));
	let mut permissions = fs::metadata(&path)
		.unwrap_or_else(|error| panic!("failed to inspect fake cargo: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&path, permissions)
		.unwrap_or_else(|error| panic!("failed to make fake cargo executable: {error}"));
	path
}

fn project_command(project: &Path, fake_cargo: &Path, target: &Path) -> Command {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(project)
		.env("CARGO", fake_cargo)
		.env("REAL_CARGO", env!("CARGO"))
		.env("CARGO_TARGET_DIR", target);
	command
}

#[test]
fn build_publishes_custom_library_artifact_and_idl() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	let args_path = temp.path().join("cargo-args.txt");
	write_project(&project);
	let cargo = fake_cargo(temp.path());

	let output = project_command(&project, &cargo, &target)
		.env("FAKE_CARGO_ARGS", &args_path)
		.args([
			"build",
			"--features",
			"logs,bpf-entrypoint",
			"--features",
			"cpi",
			"--no-default-features",
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run build command: {error}"));

	assert!(
		output.status.success(),
		"build failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(
		fs::read(target.join("deploy/custom_program.so"))
			.unwrap_or_else(|error| panic!("failed to read artifact: {error}")),
		b"compiled-sbf"
	);
	assert!(target.join("idl/custom_program.json").is_file());
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(stdout.contains("hyphen-package"));
	assert!(stdout.contains("deploy/custom_program.so"));
	assert!(stdout.contains("idl/custom_program.json"));
	let args = fs::read_to_string(&args_path)
		.unwrap_or_else(|error| panic!("failed to read Cargo arguments: {error}"));
	let args = args.lines().map(str::to_owned).collect::<Vec<_>>();
	assert_eq!(
		args,
		vec![
			"build".to_owned(),
			"--release".to_owned(),
			"--target".to_owned(),
			"bpfel-unknown-none".to_owned(),
			"--manifest-path".to_owned(),
			fs::canonicalize(project.join("Cargo.toml"))
				.unwrap_or_else(|error| panic!("failed to canonicalize manifest: {error}"))
				.to_string_lossy()
				.into_owned(),
			"-p".to_owned(),
			"hyphen-package".to_owned(),
			"-Z".to_owned(),
			"build-std".to_owned(),
			"--features".to_owned(),
			"bpf-entrypoint,cpi,logs".to_owned(),
			"--no-default-features".to_owned(),
		]
	);

	let repeated = project_command(&project, &cargo, &target)
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to repeat build command: {error}"));
	assert!(
		repeated.status.success(),
		"repeated build failed: {}",
		String::from_utf8_lossy(&repeated.stderr)
	);
	assert_eq!(
		fs::read(target.join("deploy/custom_program.so"))
			.unwrap_or_else(|error| panic!("failed to read repeated artifact: {error}")),
		b"compiled-sbf"
	);
}

#[test]
fn failed_build_does_not_publish_an_artifact() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	write_project(&project);
	let cargo = fake_cargo(temp.path());

	let output = project_command(&project, &cargo, &target)
		.env("FAKE_CARGO_FAIL", "1")
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run build command: {error}"));

	assert!(!output.status.success());
	assert!(!target.join("deploy/custom_program.so").exists());
	assert!(!target.join("idl/custom_program.json").exists());
	assert!(String::from_utf8_lossy(&output.stderr).contains("exit status: 23"));
}

#[test]
fn generate_rust_override_skips_node_and_deduplicates_clients() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	write_project(&project);
	let nested = project.join("src/instructions");
	fs::create_dir_all(&nested)
		.unwrap_or_else(|error| panic!("failed to create nested directory: {error}"));
	let cargo = fake_cargo(temp.path());

	let output = project_command(&nested, &cargo, &target)
		.args(["generate", "--client", "rust", "--client", "rust"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run generate command: {error}"));

	assert!(
		output.status.success(),
		"generation failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(project.join("clients/rust/custom_program").is_dir());
	assert!(!project.join("clients/typescript").exists());
	assert!(!project.join("clients/dart").exists());
	assert!(target.join("idl/custom_program.json").is_file());
	assert!(String::from_utf8_lossy(&output.stdout).contains("Generated 1 client(s)"));
}
