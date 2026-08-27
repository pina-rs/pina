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

fn fake_solana_verify(root: &Path) -> PathBuf {
	fs::create_dir_all(root)
		.unwrap_or_else(|error| panic!("failed to create fake verifier directory: {error}"));
	let path = root.join("fake-solana-verify.sh");
	fs::write(
		&path,
		r#"#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == "--version" ]]; then
	printf 'solana-verify %s\n' "${FAKE_VERIFY_VERSION:-0.5.1}"
	exit 0
fi

if [[ "${FAKE_VERIFY_FAIL:-0}" == "1" ]]; then
	exit 31
fi

if [[ "${FAKE_VERIFY_SIGNAL:-0}" == "1" ]]; then
	kill -TERM "$$"
fi

if [[ -n "${FAKE_VERIFY_ARGS:-}" ]]; then
	printf '%s\n' "$@" > "$FAKE_VERIFY_ARGS"
fi

workspace=""
library=""
previous=""
for argument in "$@"; do
	if [[ "$previous" == "--workspace-path" ]]; then
		workspace="$argument"
	elif [[ "$previous" == "--library-name" ]]; then
		library="$argument"
	fi
	previous="$argument"
done

mkdir -p "$workspace/target/deploy"
printf 'verified-sbf\0\0' > "$workspace/target/deploy/$library.so"

if [[ "${FAKE_VERIFY_REMOVE_SOURCE:-0}" == "1" ]]; then
	rm "$workspace/src/lib.rs"
fi
"#,
	)
	.unwrap_or_else(|error| panic!("failed to write fake verifier: {error}"));
	let mut permissions = fs::metadata(&path)
		.unwrap_or_else(|error| panic!("failed to inspect fake verifier: {error}"))
		.permissions();
	permissions.set_mode(0o755);
	fs::set_permissions(&path, permissions)
		.unwrap_or_else(|error| panic!("failed to make fake verifier executable: {error}"));
	path
}

fn commit_project(root: &Path) {
	let commands: &[&[&str]] = &[
		&["init", "-q"],
		&["config", "user.email", "pina-tests@example.com"],
		&["config", "user.name", "Pina Tests"],
		&[
			"remote",
			"add",
			"origin",
			"https://github.com/pina-rs/fixture",
		],
		&["add", "."],
		&["commit", "-qm", "fixture"],
	];
	for arguments in commands {
		let mut command = Command::new("git");
		sanitize_git_environment(&mut command);
		let status = command
			.current_dir(root)
			.args(*arguments)
			.status()
			.unwrap_or_else(|error| panic!("failed to run git {arguments:?}: {error}"));
		assert!(status.success(), "git {arguments:?} failed");
	}
}

fn sanitize_git_environment(command: &mut Command) {
	for variable in [
		"GIT_ALTERNATE_OBJECT_DIRECTORIES",
		"GIT_COMMON_DIR",
		"GIT_CONFIG",
		"GIT_CONFIG_COUNT",
		"GIT_CONFIG_PARAMETERS",
		"GIT_DIR",
		"GIT_GRAFT_FILE",
		"GIT_IMPLICIT_WORK_TREE",
		"GIT_INDEX_FILE",
		"GIT_INTERNAL_SUPER_PREFIX",
		"GIT_NO_REPLACE_OBJECTS",
		"GIT_OBJECT_DIRECTORY",
		"GIT_PREFIX",
		"GIT_REPLACE_REF_BASE",
		"GIT_SHALLOW_FILE",
		"GIT_WORK_TREE",
	] {
		command.env_remove(variable);
	}
	for (variable, _) in std::env::vars_os() {
		if variable.to_str().is_some_and(|variable| {
			variable.starts_with("GIT_CONFIG_KEY_") || variable.starts_with("GIT_CONFIG_VALUE_")
		}) {
			command.env_remove(variable);
		}
	}
}

#[test]
fn git_environment_isolation_preserves_external_config() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let sentinel = temp.path().join("sentinel.gitconfig");
	let contents = b"[user]\n\tname = Sentinel\n";
	fs::write(&sentinel, contents)
		.unwrap_or_else(|error| panic!("failed to write sentinel config: {error}"));
	let current_exe = std::env::current_exe()
		.unwrap_or_else(|error| panic!("failed to locate integration test binary: {error}"));
	let output = Command::new(current_exe)
		.args(["verified_build_", "--test-threads=1"])
		.env("GIT_CONFIG", &sentinel)
		.env("GIT_CONFIG_PARAMETERS", "malformed-injected-parameters")
		.env("GIT_CONFIG_COUNT", "1")
		.env("GIT_CONFIG_KEY_0", "user.name")
		.env("GIT_CONFIG_VALUE_0", "Injected")
		.env("GIT_DIR", "/untrusted/repository.git")
		.env("GIT_WORK_TREE", "/untrusted/worktree")
		.output()
		.unwrap_or_else(|error| panic!("failed to run isolated verified-build cases: {error}"));

	assert!(
		output.status.success(),
		"isolated verified-build cases failed:\n{}\n{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(
		fs::read(&sentinel)
			.unwrap_or_else(|error| panic!("failed to read sentinel config: {error}")),
		contents
	);
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
	sanitize_git_environment(&mut command);
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
fn verified_build_uses_solana_verify_and_publishes_hash_bound_outputs() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project with spaces");
	let target = temp.path().join("target with spaces");
	write_project(&project);
	fs::write(project.join("Cargo.lock"), "version = 4\n")
		.unwrap_or_else(|error| panic!("failed to write lockfile: {error}"));
	commit_project(&project);
	let cargo = fake_cargo(temp.path());
	let verifier = fake_solana_verify(temp.path());
	let args_path = temp.path().join("verify-args");

	let output = project_command(&project, &cargo, &target)
		.env("FAKE_VERIFY_ARGS", &args_path)
		.args([
			"build",
			"--verify",
			"--solana-verify",
			&verifier.to_string_lossy(),
			"--features",
			"logs,bpf-entrypoint",
			"--no-default-features",
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run verified build: {error}"));

	assert!(
		output.status.success(),
		"verified build failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(
		fs::read(target.join("deploy/custom_program.so"))
			.unwrap_or_else(|error| panic!("failed to read canonical artifact: {error}")),
		b"verified-sbf\0\0"
	);
	let verifiable_dir = target.join("pina/verifiable");
	let files = fs::read_dir(&verifiable_dir)
		.unwrap_or_else(|error| panic!("failed to read verifiable outputs: {error}"))
		.map(|entry| {
			entry
				.unwrap_or_else(|error| panic!("failed to read output entry: {error}"))
				.path()
		})
		.collect::<Vec<_>>();
	assert_eq!(files.len(), 2);
	let manifest_path = files
		.iter()
		.find(|path| {
			path.extension()
				.is_some_and(|extension| extension == "json")
		})
		.unwrap_or_else(|| panic!("verification manifest missing"));
	let manifest: serde_json::Value = serde_json::from_slice(
		&fs::read(manifest_path).unwrap_or_else(|error| panic!("failed to read manifest: {error}")),
	)
	.unwrap_or_else(|error| panic!("failed to parse manifest: {error}"));
	assert_eq!(manifest["schemaVersion"], 1);
	assert_eq!(manifest["libraryName"], "custom_program");
	assert_eq!(manifest["solanaVerifyVersion"], "0.5.1");
	assert_eq!(
		manifest["build"]["features"],
		serde_json::json!(["bpf-entrypoint", "logs"])
	);
	assert_eq!(manifest["build"]["defaultFeatures"], false);
	assert_eq!(manifest["source"]["dirty"], false);
	assert!(
		manifest["executableHash"]
			.as_str()
			.is_some_and(|hash| hash.len() == 64)
	);
	let record = pina_cli::build::read_verified_build_record(manifest_path)
		.unwrap_or_else(|error| panic!("failed to validate build record: {error}"));
	assert_eq!(record.library_name(), "custom_program");
	assert_eq!(
		record.repository(),
		Some("https://github.com/pina-rs/fixture")
	);
	assert_eq!(record.revision().map(str::len), Some(40));
	assert_eq!(record.mount_path(), ".");
	assert_eq!(record.workspace_path(), ".");
	assert_eq!(record.program_path(), ".");
	assert_eq!(
		record
			.artifact()
			.extension()
			.and_then(|value| value.to_str()),
		Some("so")
	);
	assert_eq!(record.executable_hash(), manifest["executableHash"]);
	let args = fs::read_to_string(args_path)
		.unwrap_or_else(|error| panic!("failed to read verifier args: {error}"));
	assert!(args.lines().any(|argument| argument == "--workspace-path"));
	assert!(args.lines().any(|argument| argument == "custom_program"));
	assert!(
		args.lines()
			.any(|argument| argument == "bpf-entrypoint,logs")
	);
	assert!(String::from_utf8_lossy(&output.stdout).contains("Build record"));
}

#[test]
fn build_record_reader_rejects_malformed_and_mismatched_records() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let malformed = temp.path().join("program.json");
	let hash = "0".repeat(64);
	let manifest = temp.path().join(format!("program-{hash}.json"));
	let artifact = manifest.with_extension("so");
	fs::write(&malformed, b"not json")
		.unwrap_or_else(|error| panic!("failed to write malformed record: {error}"));
	assert!(
		pina_cli::build::read_verified_build_record(&malformed)
			.unwrap_err()
			.to_string()
			.contains("Failed to parse")
	);
	fs::write(&artifact, b"artifact")
		.unwrap_or_else(|error| panic!("failed to write record artifact: {error}"));

	let source_manifest = serde_json::json!({
		"schemaVersion": 1,
		"packageName": "program",
		"libraryName": "program",
		"executableHash": hash,
		"solanaVerifyVersion": "0.5.1",
		"build": {
			"mountPath": ".",
			"workspacePath": ".",
			"programPath": ".",
			"libraryName": "program",
			"features": ["bpf-entrypoint"],
			"defaultFeatures": true,
			"cargoLockSha256": "0".repeat(64)
		},
		"source": {
			"repository": "https://github.com/pina-rs/program",
			"revision": "0000000000000000000000000000000000000000",
			"dirty": false
		},
		"diagnostics": []
	});
	fs::write(
		&manifest,
		serde_json::to_vec(&source_manifest)
			.unwrap_or_else(|error| panic!("failed to serialize fixture: {error}")),
	)
	.unwrap_or_else(|error| panic!("failed to write record: {error}"));
	assert!(
		pina_cli::build::read_verified_build_record(&manifest)
			.unwrap_err()
			.to_string()
			.contains("hash does not match")
	);
}

#[test]
fn verified_build_fails_closed_without_replacing_existing_outputs() {
	let cases = ["version", "failure", "signal", "dirty", "missing"];

	for case in cases {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let project = temp.path().join("project");
		let target = temp.path().join("target");
		write_project(&project);
		fs::write(project.join("Cargo.lock"), "version = 4\n")
			.unwrap_or_else(|error| panic!("failed to write lockfile: {error}"));
		commit_project(&project);
		let cargo = fake_cargo(temp.path());
		let verifier = fake_solana_verify(temp.path());
		fs::create_dir_all(target.join("deploy"))
			.unwrap_or_else(|error| panic!("failed to create deploy dir: {error}"));
		fs::write(target.join("deploy/custom_program.so"), b"existing")
			.unwrap_or_else(|error| panic!("failed to write existing artifact: {error}"));
		let mut command = project_command(&project, &cargo, &target);
		command.args(["build", "--verify", "--solana-verify"]);
		if case == "missing" {
			command.arg(temp.path().join("missing-verifier"));
		} else {
			command.arg(&verifier);
		}
		match case {
			"version" => {
				command.env("FAKE_VERIFY_VERSION", "0.5.0");
			}
			"failure" => {
				command.env("FAKE_VERIFY_FAIL", "1");
			}
			"signal" => {
				command.env("FAKE_VERIFY_SIGNAL", "1");
			}
			"dirty" => {
				fs::write(project.join("untracked-secret"), b"never stage this")
					.unwrap_or_else(|error| panic!("failed to dirty fixture: {error}"));
			}
			"missing" => {}
			other => panic!("unknown case: {other}"),
		}

		let output = command
			.output()
			.unwrap_or_else(|error| panic!("failed to run {case} case: {error}"));
		assert!(!output.status.success(), "{case} unexpectedly succeeded");
		assert_eq!(
			fs::read(target.join("deploy/custom_program.so"))
				.unwrap_or_else(|error| panic!("failed to read existing artifact: {error}")),
			b"existing",
			"{case} replaced the existing artifact"
		);
		assert!(
			!target.join("pina/verifiable").exists(),
			"{case} published verifiable outputs"
		);
		let stderr = String::from_utf8_lossy(&output.stderr);
		match case {
			"version" => assert!(stderr.contains("requires exactly 0.5.1")),
			"failure" => assert!(stderr.contains("failed (exit status: 31)")),
			"signal" => assert!(stderr.contains("signal")),
			"dirty" => assert!(stderr.contains("completely clean Git")),
			"missing" => assert!(stderr.contains("Failed to run")),
			other => panic!("unknown case: {other}"),
		}
	}
}

#[test]
fn verified_build_keeps_canonical_outputs_when_snapshot_idl_fails() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
	let project = temp.path().join("project");
	let target = temp.path().join("target");
	write_project(&project);
	fs::write(project.join("Cargo.lock"), "version = 4\n")
		.unwrap_or_else(|error| panic!("failed to write lockfile: {error}"));
	commit_project(&project);
	let cargo = fake_cargo(temp.path());
	let verifier = fake_solana_verify(temp.path());
	fs::create_dir_all(target.join("deploy"))
		.unwrap_or_else(|error| panic!("failed to create deploy dir: {error}"));
	fs::write(target.join("deploy/custom_program.so"), b"existing")
		.unwrap_or_else(|error| panic!("failed to write existing artifact: {error}"));

	let output = project_command(&project, &cargo, &target)
		.env("FAKE_VERIFY_REMOVE_SOURCE", "1")
		.args([
			"build",
			"--verify",
			"--solana-verify",
			&verifier.to_string_lossy(),
		])
		.output()
		.unwrap_or_else(|error| panic!("failed to run verified build: {error}"));

	assert!(!output.status.success());
	assert_eq!(
		fs::read(target.join("deploy/custom_program.so"))
			.unwrap_or_else(|error| panic!("failed to read existing artifact: {error}")),
		b"existing"
	);
	assert_eq!(
		fs::read_dir(target.join("pina/verifiable"))
			.unwrap_or_else(|error| panic!("failed to read build records: {error}"))
			.count(),
		2,
		"the immutable hash-bound build outputs remain accurate"
	);
}

#[test]
fn verified_build_reports_canonical_publication_failures() {
	#[derive(Clone, Copy)]
	enum Case {
		IdlDirectory,
		DeployDirectory,
		PublicationLock,
	}

	for case in [
		Case::IdlDirectory,
		Case::DeployDirectory,
		Case::PublicationLock,
	] {
		let temp = TempDir::new().unwrap_or_else(|error| panic!("temp dir failed: {error}"));
		let project = temp.path().join("project");
		let target = temp.path().join("target");
		write_project(&project);
		fs::write(project.join("Cargo.lock"), "version = 4\n")
			.unwrap_or_else(|error| panic!("failed to write lockfile: {error}"));
		commit_project(&project);
		let cargo = fake_cargo(temp.path());
		let verifier = fake_solana_verify(temp.path());
		fs::create_dir_all(&target)
			.unwrap_or_else(|error| panic!("failed to create target: {error}"));
		let case_name = match case {
			Case::IdlDirectory => {
				fs::write(target.join("idl"), b"file")
					.unwrap_or_else(|error| panic!("failed to block IDL directory: {error}"));
				"idl-directory"
			}
			Case::DeployDirectory => {
				fs::write(target.join("deploy"), b"file")
					.unwrap_or_else(|error| panic!("failed to block deploy directory: {error}"));
				"deploy-directory"
			}
			Case::PublicationLock => {
				fs::create_dir(target.join(".pina-build.lock"))
					.unwrap_or_else(|error| panic!("failed to block publication lock: {error}"));
				"publication-lock"
			}
		};

		let output = project_command(&project, &cargo, &target)
			.args([
				"build",
				"--verify",
				"--solana-verify",
				&verifier.to_string_lossy(),
			])
			.output()
			.unwrap_or_else(|error| panic!("failed to run {case_name}: {error}"));

		assert!(
			!output.status.success(),
			"{case_name} unexpectedly succeeded"
		);
		assert_eq!(
			fs::read_dir(target.join("pina/verifiable"))
				.unwrap_or_else(|error| panic!("failed to read build records: {error}"))
				.count(),
			2,
			"{case_name} should leave only accurate immutable outputs"
		);
	}
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
