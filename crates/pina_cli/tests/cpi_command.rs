use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use tempfile::TempDir;

fn fixture_path() -> PathBuf {
	workspace_root().join("codama/idls/vesting_program.json")
}

fn workspace_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
		.to_path_buf()
}

fn anchor_fixture_path() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anchor_counter.json")
}

#[test]
fn cpi_command_renders_a_file_into_a_standalone_crate() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output_dir = temp.path().join("vesting-cpi");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["cpi", "--idl"])
		.arg(fixture_path())
		.arg("--output")
		.arg(&output_dir)
		.output()
		.unwrap_or_else(|error| panic!("CPI command failed to launch: {error}"));

	assert!(
		output.status.success(),
		"CPI command failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(String::from_utf8_lossy(&output.stdout).contains("Generated CPI crate"));
	assert!(output_dir.join("Cargo.toml").is_file());
	assert!(
		output_dir
			.join("src/generated/instructions/initialize.rs")
			.is_file()
	);
}

#[test]
fn cpi_command_accepts_a_codama_pipeline_on_standard_input() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output_dir = temp.path().join("vesting-cpi");
	let mut child = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["cpi", "--stdin", "--output"])
		.arg(&output_dir)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap_or_else(|error| panic!("CPI command failed to launch: {error}"));
	child
		.stdin
		.take()
		.unwrap_or_else(|| panic!("stdin pipe missing"))
		.write_all(
			&fs::read(fixture_path())
				.unwrap_or_else(|error| panic!("fixture read failed: {error}")),
		)
		.unwrap_or_else(|error| panic!("stdin write failed: {error}"));
	let output = child
		.wait_with_output()
		.unwrap_or_else(|error| panic!("CPI command wait failed: {error}"));

	assert!(
		output.status.success(),
		"CPI command failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(output_dir.join("src/generated/mod.rs").is_file());
}

#[test]
fn cpi_command_exposes_source_only_and_destination_modes() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output_dir = temp.path().join("vesting-cpi");
	let command = |mode: &str| {
		Command::new(env!("CARGO_BIN_EXE_pina"))
			.args(["cpi", "--idl"])
			.arg(fixture_path())
			.args(["--output"])
			.arg(&output_dir)
			.args(["--mode", mode, "--no-scaffold"])
			.output()
			.unwrap_or_else(|error| panic!("CPI command failed to launch: {error}"))
	};

	let create = command("create");
	assert!(
		create.status.success(),
		"create failed: {}",
		String::from_utf8_lossy(&create.stderr)
	);
	assert!(output_dir.join("src/generated/mod.rs").is_file());
	assert!(!output_dir.join("Cargo.toml").exists());
	assert!(!output_dir.join("src/lib.rs").exists());

	let duplicate = command("create");
	assert!(!duplicate.status.success());
	assert!(String::from_utf8_lossy(&duplicate.stderr).contains("destination is not empty"));

	let update = command("update");
	assert!(
		update.status.success(),
		"update failed: {}",
		String::from_utf8_lossy(&update.stderr)
	);
	fs::write(output_dir.join("keep.txt"), "keep")
		.unwrap_or_else(|error| panic!("sentinel write failed: {error}"));

	let overwrite = command("overwrite");
	assert!(
		overwrite.status.success(),
		"overwrite failed: {}",
		String::from_utf8_lossy(&overwrite.stderr)
	);
	// Untracked files survive: overwrite removes only the paths the
	// tracked-files manifest records.
	assert!(output_dir.join("keep.txt").exists());
	assert!(output_dir.join("src/generated/mod.rs").is_file());
}

/// Workspace discovery must not execute a checkout-local Git before the
/// converter's PATH isolation, even when the converter subsequently fails.
#[cfg(unix)]
#[test]
fn cpi_command_never_executes_workspace_git_during_anchor_conversion() {
	use std::os::unix::fs::PermissionsExt;

	let temp = TempDir::new().expect("create temp dir");
	let temp_root = temp.path().canonicalize().expect("canonicalize temp dir");
	let workspace = temp_root.join("workspace");
	let project = workspace.join("programs/counter");
	let shadow_bin = workspace.join("node_modules/.bin");
	let trusted_bin = temp_root.join("trusted-bin");
	let marker = temp_root.join("shadow-git-executed");

	for directory in [&project, &shadow_bin, &trusted_bin, &workspace.join(".git")] {
		fs::create_dir_all(directory).expect("create fixture directory");
	}

	let shadow_git = shadow_bin.join("git");
	fs::write(
		&shadow_git,
		"#!/bin/sh\nprintf executed > \"$PINA_TEST_SHADOW_MARKER\"\nexit 1\n",
	)
	.expect("write shadow git");
	fs::set_permissions(&shadow_git, fs::Permissions::from_mode(0o755))
		.expect("make shadow git executable");

	let npx = trusted_bin.join("npx");
	fs::write(
		&npx,
		r#"#!/bin/sh
case "$PATH" in *"$PINA_TEST_WORKSPACE"*) echo 'workspace PATH survived' >&2; exit 90;; esac
case "$(pwd -P)" in "$PINA_TEST_WORKSPACE"*) echo 'workspace cwd survived' >&2; exit 91;; esac
if [ -n "$NODE_PATH" ]; then echo 'NODE_PATH survived' >&2; exit 92; fi
if [ "$PINA_TEST_CONVERTER_STATUS" = 7 ]; then echo 'converter failed deliberately' >&2; exit 7; fi
/bin/cat "$PINA_TEST_CONVERTED_IDL"
"#,
	)
	.expect("write trusted npx");
	fs::set_permissions(&npx, fs::Permissions::from_mode(0o755))
		.expect("make trusted npx executable");
	let inherited = std::env::var_os("PATH").unwrap_or_default();
	let path =
		std::env::join_paths(std::iter::once(shadow_bin).chain(std::env::split_paths(&inherited)))
			.expect("assemble shadowed PATH");

	for status in [0, 7] {
		let output_dir = temp_root.join(format!("output-{status}"));
		let output = Command::new(env!("CARGO_BIN_EXE_pina"))
			.args(["cpi", "--idl"])
			.arg(anchor_fixture_path())
			.arg("--npx")
			.arg(&npx)
			.arg("--output")
			.arg(&output_dir)
			.current_dir(&project)
			.env("PATH", &path)
			.env("NODE_PATH", workspace.join("node_modules"))
			.env("PINA_TEST_WORKSPACE", &workspace)
			.env("PINA_TEST_SHADOW_MARKER", &marker)
			.env("PINA_TEST_CONVERTED_IDL", fixture_path())
			.env("PINA_TEST_CONVERTER_STATUS", status.to_string())
			.output()
			.expect("run Anchor CPI command");

		assert!(!marker.exists(), "workspace Git executed before isolation");

		if status == 0 {
			assert!(
				output.status.success(),
				"conversion failed: {}",
				String::from_utf8_lossy(&output.stderr)
			);
			assert!(output_dir.join("src/generated/mod.rs").is_file());
		} else {
			assert!(!output.status.success());
			assert!(
				String::from_utf8_lossy(&output.stderr).contains("converter failed deliberately")
			);
			assert!(!output_dir.exists());
		}
	}
}

#[cfg(unix)]
#[test]
fn cpi_command_converts_a_raw_anchor_idl_and_compiles_the_crate() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let output_dir = temp.path().join("anchor-counter-cpi");
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["cpi", "--idl"])
		.arg(anchor_fixture_path())
		.args(["--npx", "node", "--output"])
		.arg(&output_dir)
		.current_dir(workspace_root())
		.output()
		.unwrap_or_else(|error| panic!("Anchor CPI command failed to launch: {error}"));

	assert!(
		output.status.success(),
		"Anchor CPI command failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let instruction_path = output_dir.join("src/generated/instructions/set_value.rs");
	let instruction = fs::read_to_string(&instruction_path)
		.unwrap_or_else(|error| panic!("generated instruction read failed: {error}"));
	assert!(instruction.contains("Set the stored counter value."));
	assert!(instruction.contains("CPI account `counter`."));
	assert!(instruction.contains("Counter account to update."));
	assert!(instruction.contains("Instruction argument `value`."));
	assert!(instruction.contains("New counter value."));
	assert!(instruction.contains("pub value: u64"));
	assert!(instruction.contains("Instruction argument `delta`."));
	assert!(instruction.contains("Signed adjustment applied after the value is stored."));
	assert!(instruction.contains("pub delta: i64"));
	assert!(instruction.contains("pub owner: &'argument Address"));
	assert!(instruction.contains("pub ix: SetValueIx<'argument>"));
	assert!(instruction.contains("pub struct SetValueIx<'argument>"));
	assert!(instruction.contains("pub fn invoke(&self, program: &ProgramAccount<'_>)"));
	assert!(instruction.contains("pub fn invoke_signed("));

	let pina_path = workspace_root().join("crates/pina");
	let manifest = format!(
		"[package]\nname = \"anchor-counter-cpi-proof\"\nversion = \"0.0.0\"\nedition = \
		 \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\npina = {{ path = {:?}, \
		 default-features = false }}\n",
		pina_path
	);
	fs::write(output_dir.join("Cargo.toml"), manifest)
		.unwrap_or_else(|error| panic!("generated manifest rewrite failed: {error}"));
	let mut check = Command::new("cargo");
	check
		.args(["check", "--quiet", "--manifest-path"])
		.arg(output_dir.join("Cargo.toml"))
		.args(["--target-dir"])
		.arg(temp.path().join("cargo-target"))
		.current_dir(workspace_root());
	sanitize_coverage_environment(&mut check);
	let check = check
		.output()
		.unwrap_or_else(|error| panic!("generated crate check failed to launch: {error}"));
	assert!(
		check.status.success(),
		"generated Anchor CPI crate did not compile: {}",
		String::from_utf8_lossy(&check.stderr)
	);
}

/// Strip the instrumentation environment inherited from a parent
/// `cargo llvm-cov` run.
///
/// The nested `cargo check` below compiles a crate that path-depends on `pina`
/// while `cargo-llvm-cov` has exported itself as `RUSTC_WRAPPER`. Left in place,
/// every crate the nested build compiles is instrumented in turn, and the
/// wrapper re-enters a full parallel cargo build for each one — the fan-out
/// grows instead of converging and exhausts the machine. The nested build only
/// needs to prove the generated crate compiles, so it runs uninstrumented.
fn sanitize_coverage_environment(command: &mut Command) {
	for variable in [
		"CARGO_ENCODED_RUSTFLAGS",
		"CARGO_LLVM_COV",
		"CARGO_LLVM_COV_BUILD_DIR",
		"CARGO_LLVM_COV_SHOW_ENV",
		"CARGO_LLVM_COV_TARGET_DIR",
		"LLVM_PROFILE_FILE",
		"RUSTC_WRAPPER",
		"RUSTFLAGS",
		"__CARGO_LLVM_COV_RUSTC_WRAPPER",
		"__CARGO_LLVM_COV_RUSTC_WRAPPER_CRATE_NAMES",
		"__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS",
	] {
		command.env_remove(variable);
	}
}

#[test]
fn cpi_command_rejects_missing_and_conflicting_inputs() {
	let temp = TempDir::new().unwrap_or_else(|error| panic!("temp failed: {error}"));
	let missing = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["cpi", "--output"])
		.arg(temp.path())
		.output()
		.unwrap_or_else(|error| panic!("missing-input command failed to launch: {error}"));
	assert!(!missing.status.success());

	let conflict = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["cpi", "--stdin", "--idl"])
		.arg(fixture_path())
		.arg("--output")
		.arg(temp.path())
		.output()
		.unwrap_or_else(|error| panic!("conflict command failed to launch: {error}"));
	assert!(!conflict.status.success());
}
