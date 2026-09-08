use std::path::Path;
use std::process::Command;

#[test]
fn cli_generation_options_drive_rendering() {
	let unique = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos();
	let output = std::env::temp_dir().join(format!("pina-renderer-cli-{unique}"));
	let idl =
		Path::new(env!("CARGO_MANIFEST_DIR")).join("../../codama/idls/hello_solana_program.json");
	let result = Command::new(env!("CARGO_BIN_EXE_pina_codama_renderer"))
		.args([
			"--idl",
			&idl.to_string_lossy(),
			"--output",
			&output.to_string_lossy(),
			"--mode",
			"create",
			"--no-scaffold",
		])
		.output()
		.unwrap_or_else(|error| panic!("renderer should start: {error}"));

	assert!(
		result.status.success(),
		"{}",
		String::from_utf8_lossy(&result.stderr)
	);
	assert!(
		output
			.join("hello_solana_program/src/generated/mod.rs")
			.is_file()
	);
	assert!(!output.join("hello_solana_program/Cargo.toml").exists());

	std::fs::remove_dir_all(output)
		.unwrap_or_else(|error| panic!("failed to clean generated client: {error}"));
}
