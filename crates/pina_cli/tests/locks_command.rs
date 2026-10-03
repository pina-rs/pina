//! `pina locks` command-line behavior: output streams, JSON, and the CI gate.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Output;

use insta_cmd::assert_cmd_snapshot;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
}

fn pina(args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_pina"))
		.current_dir(workspace_root())
		.args(args)
		.output()
		.unwrap_or_else(|error| panic!("failed to run pina {args:?}: {error}"))
}

/// A project directory whose `pina.toml` points at the multisig example and
/// carries `locks_table`.
fn multisig_project(name: &str, locks_table: &str) -> PathBuf {
	let directory = workspace_root()
		.join("target")
		.join("pina-locks-command")
		.join(name);
	fs::create_dir_all(&directory)
		.unwrap_or_else(|error| panic!("failed to create {}: {error}", directory.display()));
	fs::write(
		directory.join("pina.toml"),
		format!("[project]\nprogram = \"../../../examples/multisig_program\"\n\n{locks_table}"),
	)
	.unwrap_or_else(|error| panic!("failed to write pina.toml: {error}"));

	directory
}

#[test]
fn text_report_snapshot() {
	let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
	command
		.current_dir(workspace_root())
		.args(["locks", "--project", "examples/counter_program"]);
	assert_cmd_snapshot!("locks_counter_text", command);
}

#[test]
fn json_is_the_only_stdout_content() {
	let output = pina(&["locks", "--project", "examples/escrow_program", "--json"]);

	assert!(output.status.success());
	let json = serde_json::from_slice::<serde_json::Value>(&output.stdout)
		.unwrap_or_else(|error| panic!("locks stdout was not JSON: {error}"));
	assert_eq!(json["schemaVersion"], 1);
	assert_eq!(json["program"], "escrow_program");
	assert!(output.stderr.is_empty());
}

#[test]
fn deny_hotspots_fails_only_on_hotspots_that_are_not_allowed() {
	let denied = pina(&[
		"locks",
		"--project",
		"examples/multisig_program",
		"--deny-hotspots",
	]);
	assert_eq!(denied.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&denied.stdout).contains("program_config"));
	assert!(
		String::from_utf8_lossy(&denied.stderr)
			.contains("1 hotspot(s) are not listed in `[locks] allow`")
	);

	let quiet = pina(&[
		"locks",
		"--project",
		"examples/counter_program",
		"--deny-hotspots",
	]);
	assert!(quiet.status.success());

	let project = multisig_project("allowed", "[locks]\nallow = [\"program_config\"]\n");
	let allowed = pina(&[
		"locks",
		"--project",
		&project.to_string_lossy(),
		"--deny-hotspots",
	]);
	assert!(
		allowed.status.success(),
		"{}",
		String::from_utf8_lossy(&allowed.stderr)
	);
	assert!(String::from_utf8_lossy(&allowed.stdout).contains("(allowed in pina.toml)"));
}

#[test]
fn unknown_allow_entries_fail_with_the_hotspots_they_could_name() {
	let project = multisig_project("misspelled", "[locks]\nallow = [\"programconfig\"]\n");
	let output = pina(&["locks", "--project", &project.to_string_lossy()]);
	let stderr = String::from_utf8_lossy(&output.stderr);

	assert_eq!(output.status.code(), Some(1));
	assert!(output.stdout.is_empty());
	assert!(stderr.contains("`programconfig`"), "{stderr}");
	assert!(stderr.contains("hotspots: program_config"), "{stderr}");
}
