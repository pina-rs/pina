//! Regression coverage for IDL filenames used as output directories.

use std::path::Path;
use std::process::Command;

#[test]
fn rejects_parent_directory_stems_before_writing() {
	for directory_input in [false, true] {
		let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
		let idls = temp.path().join("idls");
		let parent = temp.path().join("destination");
		let output = parent.join("clients");
		std::fs::create_dir_all(&idls).unwrap_or_else(|error| panic!("create IDLs: {error}"));
		std::fs::create_dir_all(&output).unwrap_or_else(|error| panic!("create output: {error}"));
		std::fs::write(parent.join(".pina-generated"), "parent marker\n")
			.unwrap_or_else(|error| panic!("write parent marker: {error}"));

		let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../codama/idls/hello_solana_program.json");
		let malicious = idls.join("...json");
		let ordinary = idls.join("hello.json");

		for path in [&malicious, &ordinary] {
			std::fs::copy(&fixture, path).unwrap_or_else(|error| panic!("copy IDL: {error}"));
		}

		let mut command = Command::new(env!("CARGO_BIN_EXE_pina_cpi_renderer"));

		if directory_input {
			command.arg("--idl-dir").arg(&idls);
		} else {
			command
				.arg("--idl")
				.arg(&malicious)
				.arg("--idl")
				.arg(&ordinary);
		}

		let result = command
			.arg("--output")
			.arg(&output)
			.output()
			.unwrap_or_else(|error| panic!("start renderer: {error}"));
		let stderr = String::from_utf8_lossy(&result.stderr);
		assert!(!result.status.success(), "unsafe stem must fail: {stderr}");
		assert!(stderr.contains("child output directory"), "{stderr}");
		assert!(!parent.join("Cargo.toml").exists());
		assert!(!parent.join("src").exists());
		assert_eq!(
			std::fs::read_to_string(parent.join(".pina-generated"))
				.unwrap_or_else(|error| panic!("read parent marker: {error}")),
			"parent marker\n"
		);
		assert_eq!(
			std::fs::read_dir(&output)
				.unwrap_or_else(|error| panic!("read output: {error}"))
				.count(),
			0
		);
	}
}
