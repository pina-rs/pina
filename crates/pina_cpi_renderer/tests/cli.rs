//! Exercises the renderer binary the way a caller does: by spawning it.
//!
//! Coverage counts the spawned process, so these tests also carry the
//! binary's own lines through the patch gate.

use std::path::Path;
use std::process::Command;

fn fixture() -> String {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("../../codama/idls/hello_solana_program.json")
		.to_string_lossy()
		.into_owned()
}

fn unique_dir(prefix: &str) -> String {
	let unique = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos();
	format!("{prefix}-{unique}")
}

fn run(arguments: &[&str]) -> (bool, String, String) {
	let result = Command::new(env!("CARGO_BIN_EXE_pina_cpi_renderer"))
		.args(arguments)
		.output()
		.unwrap_or_else(|error| panic!("renderer should start: {error}"));

	(
		result.status.success(),
		String::from_utf8_lossy(&result.stdout).into_owned(),
		String::from_utf8_lossy(&result.stderr).into_owned(),
	)
}

#[test]
fn renders_a_single_idl_into_a_standalone_crate() {
	let output = unique_dir("pina-cpi-cli");

	let (success, stdout, stderr) = run(&[
		"--idl",
		&fixture(),
		"--output",
		&output,
		"--mode",
		"create",
		"--package-name",
		"counter",
	]);
	assert!(success, "{stderr}");
	assert!(stdout.contains("rendered"));
	assert!(Path::new(&output).join("Cargo.toml").is_file());
	assert!(
		Path::new(&output)
			.join("src/generated/instructions/hello.rs")
			.is_file()
	);

	std::fs::remove_dir_all(&output)
		.unwrap_or_else(|error| panic!("failed to clean generated crate: {error}"));
}

#[test]
fn renders_a_directory_of_idls_into_sibling_crates() {
	let root = unique_dir("pina-cpi-cli-dir");
	let idl_dir = Path::new(&root).join("idls");
	std::fs::create_dir_all(&idl_dir)
		.unwrap_or_else(|error| panic!("failed to create idl dir: {error}"));

	for name in ["hello_solana_program", "counter_program"] {
		let source = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../codama/idls")
			.join(format!("{name}.json"));
		std::fs::copy(&source, idl_dir.join(format!("{name}.json")))
			.unwrap_or_else(|error| panic!("failed to copy idl: {error}"));
	}

	let output = Path::new(&root).join("clients");
	let (success, _, stderr) = run(&[
		"--idl-dir",
		&idl_dir.to_string_lossy(),
		"--output",
		&output.to_string_lossy(),
	]);
	assert!(success, "{stderr}");

	assert!(
		Path::new(&output)
			.join("hello_solana_program/src/generated/mod.rs")
			.is_file()
	);
	assert!(
		Path::new(&output)
			.join("counter_program/src/generated/mod.rs")
			.is_file()
	);

	std::fs::remove_dir_all(&root)
		.unwrap_or_else(|error| panic!("failed to clean generated crates: {error}"));
}

#[test]
fn rejects_a_missing_idl_and_an_unusable_output_path() {
	let (success, _, stderr) = run(&["--idl", "./missing.json", "--output", "./unused"]);
	assert!(!success);
	assert!(stderr.contains("failed to read") || stderr.contains("failed to render"));
}

#[test]
fn rejects_invocations_without_an_idl_source() {
	let output = unique_dir("pina-cpi-cli-empty");
	let (success, _, stderr) = run(&["--output", &output]);
	assert!(!success);
	assert!(stderr.contains("provide at least one --idl"));
}
