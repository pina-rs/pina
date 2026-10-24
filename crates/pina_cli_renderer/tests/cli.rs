//! Regression coverage for IDL filenames used as output directories.

use std::path::Path;
use std::process::Command;

#[test]
fn renders_an_ordinary_idl_into_a_child_crate() {
	let temp = tempfile::tempdir().unwrap();
	let fixture =
		Path::new(env!("CARGO_MANIFEST_DIR")).join("../../codama/idls/hello_solana_program.json");
	let result = Command::new(env!("CARGO_BIN_EXE_pina_cli_renderer"))
		.arg("--idl")
		.arg(&fixture)
		.arg("--output")
		.arg(temp.path())
		.output()
		.unwrap();
	assert!(
		result.status.success(),
		"{}",
		String::from_utf8_lossy(&result.stderr)
	);
	assert!(
		temp.path()
			.join("hello_solana_program/Cargo.toml")
			.is_file()
	);
	assert!(
		temp.path()
			.join("hello_solana_program/src/main.rs")
			.is_file()
	);

	let result = Command::new(env!("CARGO_BIN_EXE_pina_cli_renderer"))
		.arg("--idl")
		.arg(&fixture)
		.arg("--output")
		.arg(temp.path())
		.args([
			"--client-path",
			"../../rust/",
			"--client-package",
			"hello-client",
		])
		.output()
		.unwrap();
	assert!(
		result.status.success(),
		"{}",
		String::from_utf8_lossy(&result.stderr)
	);
}

#[test]
fn rejects_parent_directory_stems_before_writing() {
	for directory_input in [false, true] {
		let temp = tempfile::tempdir().unwrap();
		let idls = temp.path().join("idls");
		let parent = temp.path().join("destination");
		let output = parent.join("clients");
		std::fs::create_dir_all(&idls).unwrap();
		std::fs::create_dir_all(&output).unwrap();
		std::fs::write(parent.join(".pina-generated"), "parent marker\n").unwrap();

		let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../codama/idls/hello_solana_program.json");
		let malicious = idls.join("...json");
		let ordinary = idls.join("hello.json");

		for path in [&malicious, &ordinary] {
			std::fs::copy(&fixture, path).unwrap();
		}

		let mut command = Command::new(env!("CARGO_BIN_EXE_pina_cli_renderer"));

		if directory_input {
			command.arg("--idl-dir").arg(&idls);
		} else {
			command
				.arg("--idl")
				.arg(&malicious)
				.arg("--idl")
				.arg(&ordinary);
		}

		let result = command.arg("--output").arg(&output).output().unwrap();
		let stderr = String::from_utf8_lossy(&result.stderr);
		assert!(!result.status.success(), "unsafe stem must fail: {stderr}");
		assert!(stderr.contains("child output directory"), "{stderr}");
		assert!(!parent.join("Cargo.toml").exists());
		assert!(!parent.join("src").exists());
		assert_eq!(
			std::fs::read_to_string(parent.join(".pina-generated")).unwrap(),
			"parent marker\n"
		);
		assert_eq!(std::fs::read_dir(&output).unwrap().count(), 0);
	}
}
