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
	fs::create_dir_all(root)
		.unwrap_or_else(|error| panic!("failed to create fake Cargo directory: {error}"));
	let path = root.join("fake-cargo.sh");
	fs::write(
		&path,
		r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "metadata" ]]; then
	if [[ "${FAKE_CARGO_DISAPPEAR:-0}" == "1" ]]; then
		"$REAL_CARGO" "$@"
		status="$?"
		rm "$0"
		exit "$status"
	fi
	exec "$REAL_CARGO" "$@"
fi

if [[ "${FAKE_CARGO_FAIL:-0}" == "1" ]]; then
	exit 23
fi

if [[ "${FAKE_CARGO_MISSING_ARTIFACT:-0}" == "1" ]]; then
	exit 0
fi

if [[ -n "${FAKE_CARGO_ARGS:-}" ]]; then
	printf '%s\n' "$@" > "$FAKE_CARGO_ARGS"
fi

target_dir=""
previous=""
for argument in "$@"; do
	if [[ "$previous" == "--target-dir" ]]; then
		target_dir="$argument"
		break
	fi
	previous="$argument"
done

if [[ -z "$target_dir" ]]; then
	target_dir="$CARGO_TARGET_DIR"
fi

mkdir -p "$target_dir/bpfel-unknown-none/release"
printf 'compiled-sbf' > "$target_dir/bpfel-unknown-none/release/libcustom_program.so"
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

fn fake_npx(root: &Path) -> PathBuf {
	fs::create_dir_all(root)
		.unwrap_or_else(|error| panic!("failed to create fake npx directory: {error}"));
	let path = root.join("fake-npx.sh");
	fs::write(
		&path,
		r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "${FAKE_NPX_FAIL:-0}" == "1" ]]; then
	printf 'renderer failed\n' >&2
	exit 9
fi

arguments=("$@")
for ((index = 0; index < ${#arguments[@]}; index++)); do
	if [[ "${arguments[$index]}" != "typescript" && "${arguments[$index]}" != "dart" ]]; then
		continue
	fi

	renderer="${arguments[$index]}"
	output="${arguments[$((index + 1))]}"
	idl="${arguments[$((index + 2))]}"
	name="$(basename "$idl" .json)"
	if [[ "$renderer" == "typescript" ]]; then
		mkdir -p "$output/$name/src/generated"
		printf '{"name":"%s-client"}\n' "${name//_/-}" > "$output/$name/package.json"
	else
		mkdir -p "$output/lib/src/generated/$name"
		printf '// generated\n' > "$output/lib/src/generated/$name/$name.dart"
	fi
	exit 0
done

exit 2
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write fake npx: {error}"));
	let mut permissions = fs::metadata(&path)
		.unwrap_or_else(|error| panic!("failed to inspect fake npx: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&path, permissions)
		.unwrap_or_else(|error| panic!("failed to make fake npx executable: {error}"));
	path
}

fn generated_idl_name(path: &Path) -> String {
	let json = fs::read(path)
		.unwrap_or_else(|error| panic!("failed to read generated IDL {}: {error}", path.display()));
	let value: serde_json::Value = serde_json::from_slice(&json)
		.unwrap_or_else(|error| panic!("failed to parse generated IDL: {error}"));

	value["program"]["name"]
		.as_str()
		.unwrap_or_else(|| panic!("generated IDL did not contain program.name"))
		.to_owned()
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
	assert_eq!(
		generated_idl_name(&target.join("idl/custom_program.json")),
		"customProgram"
	);
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
			"--target-dir".to_owned(),
			target.to_string_lossy().into_owned(),
			"--manifest-path".to_owned(),
			fs::canonicalize(project.join("Cargo.toml"))
				.unwrap_or_else(|error| panic!("failed to canonicalize manifest: {error}"))
				.to_string_lossy()
				.into_owned(),
			"-p".to_owned(),
			"hyphen-package".to_owned(),
			"-Z".to_owned(),
			"build-std=core,alloc".to_owned(),
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
fn build_resolves_relative_target_from_config_root_for_subdirectory_program() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let root = temp.path().join("project");
	let program = root.join("programs/custom");
	let target = root.join("relative-target");
	write_project(&program);
	fs::remove_file(program.join("Pina.toml"))
		.unwrap_or_else(|error| panic!("failed to remove nested config: {error}"));
	fs::write(
		root.join("Pina.toml"),
		r#"[project]
program = "programs/custom"

[clients]
output = "clients"
languages = ["rust"]
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write root config: {error}"));
	let cargo = fake_cargo(temp.path());

	let output = project_command(&root, &cargo, Path::new("relative-target"))
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run build command: {error}"));

	assert!(
		output.status.success(),
		"build failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(target.join("deploy/custom_program.so").is_file());
	assert!(target.join("idl/custom_program.json").is_file());
	assert_eq!(
		generated_idl_name(&target.join("idl/custom_program.json")),
		"customProgram"
	);
}

#[test]
fn build_uses_metadata_target_from_configless_nested_start() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let nested = project.join("src/instructions");
	write_project(&project);
	fs::remove_file(project.join("Pina.toml"))
		.unwrap_or_else(|error| panic!("failed to remove config: {error}"));
	fs::create_dir_all(&nested)
		.unwrap_or_else(|error| panic!("failed to create nested directory: {error}"));
	let cargo = fake_cargo(temp.path());
	let target = nested.join("relative-target");

	let output = project_command(&nested, &cargo, Path::new("relative-target"))
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run build command: {error}"));

	assert!(
		output.status.success(),
		"build failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(target.join("deploy/custom_program.so").is_file());
	assert!(target.join("idl/custom_program.json").is_file());
	assert_eq!(
		generated_idl_name(&target.join("idl/custom_program.json")),
		"customProgram"
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
		.args(["build", "--features", "logs", "--no-default-features"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run build command: {error}"));

	assert!(!output.status.success());
	assert!(!target.join("deploy/custom_program.so").exists());
	assert!(!target.join("idl/custom_program.json").exists());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("exit status: 23"));
	assert!(stderr.contains("build-std=core,alloc"));
	assert!(stderr.contains("--target-dir"));
	assert!(stderr.contains("--manifest-path"));
	assert!(stderr.contains("--features"));
	assert!(stderr.contains("bpf-entrypoint,logs"));
	assert!(stderr.contains("--no-default-features"));
}

#[test]
fn build_reports_missing_cargo_and_compiler_artifacts() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	write_project(&project);
	let disappearing_cargo = fake_cargo(&temp.path().join("disappearing"));
	let output = project_command(&project, &disappearing_cargo, &target)
		.env("FAKE_CARGO_DISAPPEAR", "1")
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run command: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("Failed to run"));

	let cargo = fake_cargo(&temp.path().join("missing-artifact"));
	let output = project_command(&project, &cargo, &target)
		.env("FAKE_CARGO_MISSING_ARTIFACT", "1")
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run command: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("was not created"));
}

#[test]
fn build_reports_idl_and_output_directory_failures() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	write_project(&project);
	let cargo = fake_cargo(temp.path());

	fs::write(project.join("src/lib.rs"), "this is not Rust")
		.unwrap_or_else(|error| panic!("failed to corrupt source: {error}"));
	let output = project_command(&project, &cargo, &target)
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run command: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("IDL generation failed"));

	write_project(&project);
	fs::write(
		project.join("Pina.toml"),
		"[project]\nprogram = \".\"\nidl_dir = \"blocked-idl\"\n",
	)
	.unwrap_or_else(|error| panic!("failed to update config: {error}"));
	fs::write(project.join("blocked-idl"), b"file")
		.unwrap_or_else(|error| panic!("failed to block IDL directory: {error}"));
	let output = project_command(&project, &cargo, &target)
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run command: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("Failed to create IDL directory"));

	fs::remove_file(project.join("blocked-idl"))
		.unwrap_or_else(|error| panic!("failed to unblock IDL directory: {error}"));
	fs::write(project.join("Pina.toml"), "")
		.unwrap_or_else(|error| panic!("failed to reset config: {error}"));
	fs::write(target.join("deploy"), b"file")
		.unwrap_or_else(|error| panic!("failed to block deploy directory: {error}"));
	let output = project_command(&project, &cargo, &target)
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run command: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("artifact directory"));

	fs::remove_file(target.join("deploy"))
		.unwrap_or_else(|error| panic!("failed to unblock deploy directory: {error}"));
	fs::create_dir_all(target.join("idl/custom_program.json"))
		.unwrap_or_else(|error| panic!("failed to block IDL file: {error}"));
	let output = project_command(&project, &cargo, &target)
		.arg("build")
		.output()
		.unwrap_or_else(|error| panic!("failed to run command: {error}"));
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("before publication"));
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
	let rust_manifest = fs::read_to_string(project.join("clients/rust/custom_program/Cargo.toml"))
		.unwrap_or_else(|error| panic!("failed to read generated Rust manifest: {error}"));
	assert!(rust_manifest.contains("name = \"custom-program-client\""));
	assert!(!project.join("clients/typescript").exists());
	assert!(!project.join("clients/dart").exists());
	assert!(target.join("idl/custom_program.json").is_file());
	assert!(String::from_utf8_lossy(&output.stdout).contains("Generated 1 client(s)"));
}

#[test]
fn generate_typescript_uses_custom_library_identity() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	write_project(&project);
	let cargo = fake_cargo(temp.path());
	let npx = fake_npx(temp.path());

	let output = project_command(&project, &cargo, &target)
		.args([
			"generate",
			"--client",
			"typescript",
			"--npx",
			npx.to_string_lossy().as_ref(),
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run generation: {error}"));

	assert!(
		output.status.success(),
		"generation failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(
		generated_idl_name(&target.join("idl/custom_program.json")),
		"customProgram"
	);
	let package =
		fs::read_to_string(project.join("clients/typescript/custom_program/package.json"))
			.unwrap_or_else(|error| panic!("failed to read TypeScript package: {error}"));
	assert!(package.contains("custom-program-client"));
}

#[test]
fn generate_uses_configured_clients_and_supports_dart() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	write_project(&project);
	fs::write(
		project.join("Pina.toml"),
		"[clients]\noutput = \"clients\"\nlanguages = [\"typescript\", \"dart\"]\n",
	)
	.unwrap_or_else(|error| panic!("failed to configure clients: {error}"));
	let cargo = fake_cargo(temp.path());
	let npx = fake_npx(temp.path());

	let output = project_command(&project, &cargo, &target)
		.arg("generate")
		.arg("--npx")
		.arg(&npx)
		.output()
		.unwrap_or_else(|error| panic!("failed to run generation: {error}"));

	assert!(
		output.status.success(),
		"generation failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		project
			.join("clients/typescript/custom_program/package.json")
			.is_file()
	);
	assert!(
		project
			.join("clients/dart/lib/custom_program.dart")
			.is_file()
	);
}

#[test]
fn generate_reports_project_discovery_errors() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(temp.path())
		.args(["generate", "--client", "dart"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run generation: {error}"));

	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stderr).contains("Cargo metadata discovery failed"));
}

#[test]
fn generate_falls_back_from_missing_npx_to_pnpm() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("custom-target");
	let tools = temp.path().join("tools");
	write_project(&project);
	let cargo = fake_cargo(temp.path());
	let renderer = fake_npx(&tools);
	fs::rename(&renderer, tools.join("pnpm"))
		.unwrap_or_else(|error| panic!("failed to install fake pnpm: {error}"));

	let output = project_command(&project, &cargo, &target)
		.env("PATH", format!("{}:/bin:/usr/bin", tools.display()))
		.args(["generate", "--client", "typescript"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run generation: {error}"));

	assert!(
		output.status.success(),
		"generation failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		project
			.join("clients/typescript/custom_program/package.json")
			.is_file()
	);

	let failed = project_command(&project, &cargo, &target)
		.env("PATH", format!("{}:/bin:/usr/bin", tools.display()))
		.env("FAKE_NPX_FAIL", "1")
		.args(["generate", "--client", "typescript"])
		.output()
		.unwrap_or_else(|error| panic!("failed to run failing generation: {error}"));
	assert!(!failed.status.success());
	assert!(String::from_utf8_lossy(&failed.stderr).contains("npx (or fallback pnpm dlx)"));
}
