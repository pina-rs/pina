//! Regression coverage for IDL filenames used as output directories.

use std::path::Path;
use std::process::Command;

#[test]
fn rejects_parent_directory_stems_before_writing() {
	for (malicious_name, directory_input, unsafe_first) in [
		("...json", false, true),
		("...json", false, false),
		("...json", true, true),
		#[cfg(unix)]
		("z:name.json", false, false),
		#[cfg(unix)]
		("z:name.json", true, false),
	] {
		let temp = tempfile::tempdir().unwrap();
		let idls = temp.path().join("idls");
		let parent = temp.path().join("destination");
		let output = parent.join("clients");
		std::fs::create_dir_all(&idls).unwrap();
		std::fs::create_dir_all(&output).unwrap();
		std::fs::write(parent.join(".pina-generated"), "parent marker\n").unwrap();

		let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
			.join("../../codama/idls/hello_solana_program.json");
		let malicious = idls.join(malicious_name);
		let ordinary = idls.join("hello.json");

		for path in [&malicious, &ordinary] {
			std::fs::copy(&fixture, path).unwrap();
		}

		let mut command = Command::new(env!("CARGO_BIN_EXE_pina_cpi_renderer"));

		if directory_input {
			command.arg("--idl-dir").arg(&idls);
		} else {
			let explicit_idls = if unsafe_first {
				[&malicious, &ordinary]
			} else {
				[&ordinary, &malicious]
			};
			for idl in explicit_idls {
				command.arg("--idl").arg(idl);
			}
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
