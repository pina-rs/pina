use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use tempfile::TempDir;

fn fixture_path() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
		.join("codama/idls/vesting_program.json")
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
