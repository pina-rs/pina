//! Integration tests for the `pina_profile` crate.
//!
//! These tests build minimal synthetic SBF ELF binaries using the `object`
//! crate's write module, write them to temp files, and exercise the full
//! `profile_program` pipeline end-to-end.

use std::io::Write;
use std::path::Path;

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
use pina_profile::OutputFormat;

/// Build a minimal SBF ELF binary with a `.text` section and optional symbols.
fn build_sbf_elf(text_size: usize, symbols: &[(&str, u64, u64)]) -> Vec<u8> {
	let mut obj = Object::new(BinaryFormat::Elf, Architecture::Sbf, Endianness::Little);

	let section = obj.add_section(Vec::new(), b".text".to_vec(), SectionKind::Text);

	// Fill .text with NOP-like bytes (0x00 repeated).
	let text_data = vec![0u8; text_size];
	obj.set_section_data(section, text_data, 8);

	for &(name, offset, size) in symbols {
		let sym_id = obj.add_symbol(Symbol {
			name: name.as_bytes().to_vec(),
			value: offset,
			size,
			kind: SymbolKind::Text,
			scope: SymbolScope::Dynamic,
			weak: false,
			section: SymbolSection::Section(section),
			flags: SymbolFlags::None,
		});
		let _ = sym_id;
	}

	obj.write()
		.unwrap_or_else(|e| panic!("failed to write ELF: {e}"))
}

/// Write bytes to a temp file and return the path.
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

// =========================================================================
// End-to-end profile_program tests
// =========================================================================

#[test]
fn profile_program_with_symbols() {
	let elf_data = build_sbf_elf(160, &[("entrypoint", 0, 80), ("helper", 80, 80)]);
	let file = write_temp_elf(&elf_data);

	let profile = pina_profile::profile_program(file.path())
		.unwrap_or_else(|e| panic!("profile failed: {e}"));

	assert_eq!(profile.text_size, 160);
	assert_eq!(profile.total_instructions, 20); // 160 / 8
	assert_eq!(profile.total_cu, 20);
	assert!(!profile.functions.is_empty());

	// Both symbols should be present.
	let names: Vec<&str> = profile.functions.iter().map(|f| f.name.as_str()).collect();
	assert!(
		names.contains(&"entrypoint"),
		"missing entrypoint: {names:?}"
	);
	assert!(names.contains(&"helper"), "missing helper: {names:?}");
}

#[test]
fn profile_program_no_symbols() {
	let elf_data = build_sbf_elf(80, &[]);
	let file = write_temp_elf(&elf_data);

	let profile = pina_profile::profile_program(file.path())
		.unwrap_or_else(|e| panic!("profile failed: {e}"));

	assert_eq!(profile.text_size, 80);
	assert_eq!(profile.total_instructions, 10);
	assert_eq!(profile.functions.len(), 1);
	assert_eq!(profile.functions[0].name, "<entire .text>");
}

#[test]
fn profile_program_rejects_nonexistent_file() {
	let result = pina_profile::profile_program(Path::new("/nonexistent/path.so"));
	assert!(result.is_err());
}

#[test]
fn profile_program_rejects_non_elf() {
	let mut file = tempfile::Builder::new()
		.suffix(".so")
		.tempfile()
		.unwrap_or_else(|e| panic!("failed to create temp file: {e}"));
	file.write_all(b"this is not an ELF file")
		.unwrap_or_else(|e| panic!("write failed: {e}"));
	file.flush().unwrap_or_else(|e| panic!("flush failed: {e}"));

	let result = pina_profile::profile_program(file.path());
	assert!(result.is_err());
}

// =========================================================================
// Output format tests (end-to-end through write_profile)
// =========================================================================

#[test]
fn text_output_end_to_end() {
	let elf_data = build_sbf_elf(80, &[("my_func", 0, 80)]);
	let file = write_temp_elf(&elf_data);

	let profile = pina_profile::profile_program(file.path())
		.unwrap_or_else(|e| panic!("profile failed: {e}"));

	let mut buf = Vec::new();
	pina_profile::output::write_profile(&profile, OutputFormat::Text, &mut buf)
		.unwrap_or_else(|e| panic!("write failed: {e}"));

	let output = String::from_utf8(buf).unwrap_or_else(|e| panic!("non-UTF8 output: {e}"));
	assert!(
		output.contains("my_func"),
		"output should contain function name"
	);
	assert!(
		output.contains("Total estimated CU"),
		"output should contain CU summary"
	);
}

#[test]
fn json_output_end_to_end() {
	let elf_data = build_sbf_elf(80, &[("my_func", 0, 80)]);
	let file = write_temp_elf(&elf_data);

	let profile = pina_profile::profile_program(file.path())
		.unwrap_or_else(|e| panic!("profile failed: {e}"));

	let mut buf = Vec::new();
	pina_profile::output::write_profile(&profile, OutputFormat::Json, &mut buf)
		.unwrap_or_else(|e| panic!("write failed: {e}"));

	let output = String::from_utf8(buf).unwrap_or_else(|e| panic!("non-UTF8 output: {e}"));
	let parsed: serde_json::Value =
		serde_json::from_str(&output).unwrap_or_else(|e| panic!("invalid JSON: {e}"));

	assert_eq!(parsed["text_size"], 80);
	assert_eq!(parsed["total_instructions"], 10);
	assert!(parsed["functions"].is_array());
	assert!(parsed["functions"][0]["name"].as_str().is_some());
}
