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

#[cfg(unix)]
#[test]
fn refuses_default_and_explicit_symlink_outputs_without_touching_victims() {
	use std::os::unix::fs::symlink;

	for explicit in [false, true] {
		for linked_parent in [false, true] {
			let temp = tempfile::tempdir().unwrap();
			let target = temp.path().join("target");
			let parent = target.join("pina");
			let victim = temp.path().join("victim");
			fs::create_dir_all(&target).unwrap();
			fs::create_dir(&victim).unwrap();
			let sentinel = victim.join("map.html");
			fs::write(&sentinel, b"deployment keypair sentinel").unwrap();

			if linked_parent {
				symlink(&victim, &parent).unwrap();
			} else {
				fs::create_dir(&parent).unwrap();
				symlink(&sentinel, parent.join("map.html")).unwrap();
			}

			let mut command = Command::new(env!("CARGO_BIN_EXE_pina"));
			command
				.current_dir(workspace_root())
				.env("CARGO_TARGET_DIR", &target)
				.args(["map", "--project", "examples/counter_program"]);

			if explicit {
				command.arg("--output").arg(parent.join("map.html"));
			}

			let output = command.output().unwrap();

			assert_eq!(output.status.code(), Some(1), "{output:?}");
			assert!(output.stdout.is_empty());
			assert!(
				String::from_utf8_lossy(&output.stderr).contains("symbolic link or reparse point")
			);
			assert_eq!(fs::read(&sentinel).unwrap(), b"deployment keypair sentinel");
			assert!(
				fs::symlink_metadata(if linked_parent {
					parent
				} else {
					parent.join("map.html")
				})
				.unwrap()
				.is_symlink()
			);

			// JSON mode never publishes HTML, even when the default is hostile.
			if !explicit {
				let output = command.arg("--json").output().unwrap();
				assert!(output.status.success(), "{output:?}");
				assert!(serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok());
				assert_eq!(fs::read(&sentinel).unwrap(), b"deployment keypair sentinel");
			}
		}
	}
}
