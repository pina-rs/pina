use std::fs;
use std::path::Path;
use std::path::PathBuf;

use pina_cli::codegen::ir_to_root_node;
use pina_cli::parse::assemble_program_ir;

fn fixture_dir() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn fixture_programs_idl() {
	let mut fixtures = fs::read_dir(fixture_dir())
		.unwrap_or_else(|e| panic!("failed to read fixture directory: {e}"))
		.filter_map(|entry| entry.ok().map(|entry| entry.path()))
		.filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
		.collect::<Vec<_>>();
	fixtures.sort();

	assert!(!fixtures.is_empty(), "no fixtures found in tests/fixtures");

	for fixture in fixtures {
		let fixture_name = fixture
			.file_stem()
			.and_then(|name| name.to_str())
			.unwrap_or_else(|| panic!("invalid fixture name: {}", fixture.display()))
			.to_owned();

		let source = fs::read_to_string(&fixture)
			.unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", fixture.display()));
		let file = syn::parse_file(&source)
			.unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", fixture.display()));
		let ir = assemble_program_ir(&file, &fixture_name)
			.unwrap_or_else(|e| panic!("failed to assemble IR for {}: {e}", fixture.display()));
		let idl = ir_to_root_node(&ir);

		insta::assert_json_snapshot!(fixture_name, idl);
	}
}
