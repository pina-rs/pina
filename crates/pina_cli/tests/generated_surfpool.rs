#![cfg(not(coverage))]

use std::fs;
use std::path::Path;
use std::process::Command;

fn workspace_root() -> &'static Path {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
}

#[test]
#[ignore = "builds a real SBF program and starts an embedded OfflineSurfnet"]
fn initialized_project_executes_its_starter_instruction_on_surfpool() {
	let temporary = tempfile::tempdir()
		.unwrap_or_else(|error| panic!("failed to create generated-project directory: {error}"));
	let project = temporary.path().join("generated_program");
	let pina = env!("CARGO_BIN_EXE_pina");
	let init = Command::new(pina)
		.args(["init", "generated_program", "--path"])
		.arg(&project)
		.status()
		.unwrap_or_else(|error| panic!("failed to run pina init: {error}"));
	assert!(init.success(), "pina init failed with {init}");

	let manifest = project.join("Cargo.toml");
	let contents = fs::read_to_string(&manifest)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", manifest.display()));
	let pina_path = workspace_root().join("crates/pina");
	let contents = contents
		.lines()
		.map(|line| {
			if line.starts_with("pina = ") {
				format!(
					"pina = {{ path = \"{}\", features = [\"logs\", \"derive\"] }}",
					pina_path.display()
				)
			} else {
				line.to_owned()
			}
		})
		.collect::<Vec<_>>()
		.join("\n");
	fs::write(&manifest, format!("{contents}\n"))
		.unwrap_or_else(|error| panic!("failed to update {}: {error}", manifest.display()));

	let surfpool_manifest = project.join("tests/surfpool/Cargo.toml");
	let contents = fs::read_to_string(&surfpool_manifest)
		.unwrap_or_else(|error| panic!("failed to read {}: {error}", surfpool_manifest.display()));
	let pina_test_path = workspace_root().join("crates/pina_test");
	let contents = contents
		.lines()
		.map(|line| {
			if line.starts_with("pina_test = ") {
				format!("pina_test = {{ path = \"{}\" }}", pina_test_path.display())
			} else {
				line.to_owned()
			}
		})
		.collect::<Vec<_>>()
		.join("\n");
	fs::write(&surfpool_manifest, format!("{contents}\n")).unwrap_or_else(|error| {
		panic!("failed to update {}: {error}", surfpool_manifest.display())
	});

	let target_dir = workspace_root().join("target/generated-surfpool");
	let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
	let check = Command::new(cargo)
		.args(["check", "--all-features", "--manifest-path"])
		.arg(&manifest)
		.env("CARGO_TARGET_DIR", &target_dir)
		.status()
		.unwrap_or_else(|error| panic!("failed to check generated project: {error}"));
	assert!(
		check.success(),
		"generated all-features check failed with {check}"
	);

	let test = Command::new(pina)
		.args(["test", "--project"])
		.arg(&project)
		.env("CARGO_TARGET_DIR", target_dir)
		.status()
		.unwrap_or_else(|error| panic!("failed to run generated Surfpool test: {error}"));
	assert!(
		test.success(),
		"generated SBF/Surfpool test failed with {test}"
	);
}
