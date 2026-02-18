use std::path::Path;

use pina_cli::generate_idl;

fn workspace_root() -> &'static Path {
	// The test binary runs from the workspace root.
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(|p| p.parent())
		.unwrap_or_else(|| Path::new("."))
}

#[test]
fn counter_program_idl() {
	let root = workspace_root();
	let idl = generate_idl(&root.join("examples/counter_program"), None)
		.unwrap_or_else(|e| panic!("IDL generation failed: {e}"));
	insta::assert_json_snapshot!("counter_program", idl);
}

#[test]
fn escrow_program_idl() {
	let root = workspace_root();
	let idl = generate_idl(&root.join("examples/escrow_program"), None)
		.unwrap_or_else(|e| panic!("IDL generation failed: {e}"));
	insta::assert_json_snapshot!("escrow_program", idl);
}

#[test]
fn transfer_sol_idl() {
	let root = workspace_root();
	let idl = generate_idl(&root.join("examples/transfer_sol"), None)
		.unwrap_or_else(|e| panic!("IDL generation failed: {e}"));
	insta::assert_json_snapshot!("transfer_sol", idl);
}

#[test]
fn hello_solana_idl() {
	let root = workspace_root();
	let idl = generate_idl(&root.join("examples/hello_solana"), None)
		.unwrap_or_else(|e| panic!("IDL generation failed: {e}"));
	insta::assert_json_snapshot!("hello_solana", idl);
}
