//! Integration tests for the `pina profile` CLI command.
//!
//! Builds minimal synthetic SBF ELF binaries and exercises the CLI end-to-end.

use std::io::Write;
use std::process::Command;

use object::Architecture;
use object::BinaryFormat;
use object::Endianness;
use object::SectionKind;
use object::SymbolFlags;
use object::SymbolKind;
use object::SymbolScope;
use object::write::Object;
use object::write::Symbol;
use object::write::SymbolSection;

/// Build a minimal SBF ELF binary with a `.text` section and optional symbols.
fn build_sbf_elf(text_size: usize, symbols: &[(&str, u64, u64)]) -> Vec<u8> {
	let mut obj = Object::new(BinaryFormat::Elf, Architecture::Sbf, Endianness::Little);

	let section = obj.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);

	let text_data = vec![0u8; text_size];
	obj.set_section_data(section, text_data, 8);

	for &(name, offset, size) in symbols {
		obj.add_symbol(Symbol {
			name: name.as_bytes().to_vec(),
			value: offset,
			size,
			kind: SymbolKind::Text,
			scope: SymbolScope::Dynamic,
			weak: false,
			section: SymbolSection::Section(section),
			flags: SymbolFlags::None,
		});
	}

	obj.write()
		.unwrap_or_else(|e| panic!("failed to write ELF: {e}"))
}

/// Write bytes to a temp file and return it.
fn write_temp_elf(data: &[u8]) -> tempfile::NamedTempFile {
	let mut file = tempfile::Builder::new()
		.suffix(".so")
		.tempfile()
		.unwrap_or_else(|e| panic!("failed to create temp file: {e}"));
	file.write_all(data)
		.unwrap_or_else(|e| panic!("failed to write temp ELF: {e}"));
	file.flush()
		.unwrap_or_else(|e| panic!("failed to flush temp ELF: {e}"));
	file
}

#[test]
fn cli_profile_text_output() {
	let elf_data = build_sbf_elf(160, &[("process_instruction", 0, 160)]);
	let file = write_temp_elf(&elf_data);

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", file.path().to_str().unwrap()])
		.output()
		.unwrap_or_else(|e| panic!("failed to run pina profile: {e}"));

	assert!(
		output.status.success(),
		"pina profile failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	assert!(
		stdout.contains("process_instruction"),
		"expected function name in output: {stdout}"
	);
	assert!(
		stdout.contains("Total estimated CU"),
		"expected CU summary in output: {stdout}"
	);
}

#[test]
fn cli_profile_json_output() {
	let elf_data = build_sbf_elf(160, &[("process_instruction", 0, 160)]);
	let file = write_temp_elf(&elf_data);

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", file.path().to_str().unwrap(), "--json"])
		.output()
		.unwrap_or_else(|e| panic!("failed to run pina profile --json: {e}"));

	assert!(
		output.status.success(),
		"pina profile --json failed: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	let stdout = String::from_utf8_lossy(&output.stdout);
	let parsed: serde_json::Value =
		serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid JSON: {e}"));
	assert_eq!(parsed["total_instructions"], 20);
	assert!(parsed["functions"].is_array());
}

#[test]
fn cli_profile_output_to_file() {
	let elf_data = build_sbf_elf(80, &[("my_func", 0, 80)]);
	let elf_file = write_temp_elf(&elf_data);
	let output_file = tempfile::Builder::new()
		.suffix(".json")
		.tempfile()
		.unwrap_or_else(|e| panic!("temp: {e}"));

	let result = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args([
			"profile",
			elf_file.path().to_str().unwrap(),
			"--json",
			"--output",
			output_file.path().to_str().unwrap(),
		])
		.output()
		.unwrap_or_else(|e| panic!("failed to run pina profile --output: {e}"));

	assert!(
		result.status.success(),
		"pina profile --output failed: {}",
		String::from_utf8_lossy(&result.stderr)
	);

	let content = std::fs::read_to_string(output_file.path())
		.unwrap_or_else(|e| panic!("read output file: {e}"));
	let parsed: serde_json::Value = serde_json::from_str(&content)
		.unwrap_or_else(|e| panic!("invalid JSON in output file: {e}"));
	assert!(parsed["functions"].is_array());
}

#[test]
fn cli_profile_nonexistent_file_fails() {
	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", "/nonexistent/path.so"])
		.output()
		.unwrap_or_else(|e| panic!("failed to run: {e}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error"), "expected error message: {stderr}");
}

#[test]
fn cli_profile_non_elf_fails() {
	let mut file = tempfile::Builder::new()
		.suffix(".so")
		.tempfile()
		.unwrap_or_else(|e| panic!("temp: {e}"));
	file.write_all(b"not an elf").unwrap();
	file.flush().unwrap();

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", file.path().to_str().unwrap()])
		.output()
		.unwrap_or_else(|e| panic!("failed to run: {e}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error"), "expected error message: {stderr}");
}

#[test]
fn cli_profile_refuses_overwrite_input() {
	let elf_data = build_sbf_elf(80, &[]);
	let file = write_temp_elf(&elf_data);
	let path_str = file.path().to_str().unwrap();

	let output = Command::new(env!("CARGO_BIN_EXE_pina"))
		.args(["profile", path_str, "--output", path_str])
		.output()
		.unwrap_or_else(|e| panic!("failed to run: {e}"));

	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(
		stderr.contains("Refusing to overwrite"),
		"expected overwrite guard: {stderr}"
	);
}
