//! `pina map` command-line behavior: where the page goes and what stdout carries.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::Output;

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

fn stdout(output: &Output) -> String {
	String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn writes_the_page_to_the_requested_path_and_prints_it() {
	let path = "target/pina-map-command/nested/escrow.html";
	let _ = fs::remove_file(workspace_root().join(path));
	let output = pina(&[
		"map",
		"--project",
		"examples/escrow_program",
		"--output",
		path,
	]);

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert_eq!(stdout(&output).trim(), path);
	assert!(
		String::from_utf8_lossy(&output.stderr)
			.contains("Wrote the program map for escrow_program")
	);

	let html = fs::read_to_string(workspace_root().join(path))
		.unwrap_or_else(|error| panic!("the map was not written: {error}"));
	assert!(html.starts_with("<!doctype html>"));
	assert!(html.contains("<script type=\"application/json\" id=\"pina-map-data\">"));
}

#[test]
fn writes_the_page_under_the_target_directory_by_default() {
	let output = pina(&["map", "--project", "examples/counter_program"]);
	let printed = stdout(&output);
	let printed = Path::new(printed.trim());

	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		printed.ends_with("target/pina/map.html"),
		"{}",
		printed.display()
	);
	assert!(printed.is_file());
}

#[test]
fn json_prints_the_map_data_instead_of_writing_html() {
	let output = pina(&["map", "--project", "examples/counter_program", "--json"]);

	assert!(output.status.success());
	let json = serde_json::from_slice::<serde_json::Value>(&output.stdout)
		.unwrap_or_else(|error| panic!("map stdout was not JSON: {error}"));
	assert_eq!(json["schemaVersion"], 1);
	assert_eq!(json["instructionDetails"][0]["name"], "initialize");
	assert_eq!(json["accountTypes"][0]["name"], "CounterState");
	assert!(output.stderr.is_empty());
}

#[test]
fn reports_failures_without_writing() {
	let missing = pina(&["map", "--project", "target/definitely-missing-program"]);
	assert_eq!(missing.status.code(), Some(1));
	assert!(missing.stdout.is_empty());

	let blocked = pina(&[
		"map",
		"--project",
		"examples/counter_program",
		"--output",
		"Cargo.toml/map.html",
	]);
	assert_eq!(blocked.status.code(), Some(1));
	assert!(String::from_utf8_lossy(&blocked.stderr).contains("Could not write the program map"));

	let conflicting = pina(&["map", "--json", "--output", "map.html"]);
	assert_eq!(conflicting.status.code(), Some(2));
}
