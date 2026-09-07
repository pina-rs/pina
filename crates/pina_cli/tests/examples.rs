use std::fs;
use std::path::Path;

use codama_nodes::RootNode;
use pina_cli::generate_idl;
use pina_cli::project::Project;
use serde_json::Value;

const EXAMPLES: &[&str] = &[
	"anchor_declare_id",
	"anchor_declare_program",
	"anchor_duplicate_mutable_accounts",
	"anchor_errors",
	"anchor_events",
	"anchor_floats",
	"anchor_realloc",
	"anchor_system_accounts",
	"anchor_sysvars",
	"compact_accounts",
	"counter_program",
	"escrow_program",
	"hello_solana",
	"optional_accounts_program",
	"pina_bpf",
	"profile_program",
	"prop_amm_program",
	"role_registry_program",
	"staking_rewards_program",
	"todo_program",
	"transfer_sol",
	"vesting_program",
];

fn workspace_root() -> &'static Path {
	// The test binary runs from the workspace root.
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(|p| p.parent())
		.unwrap_or_else(|| Path::new("."))
}

fn example_program_idl(example: &str) -> RootNode {
	let root = workspace_root();
	generate_idl(&root.join("examples").join(example), None)
		.unwrap_or_else(|e| panic!("IDL generation failed for {example}: {e}"))
}

fn assert_matches_committed_idl(example: &str) {
	let root = workspace_root();
	let committed_path = root.join("codama/idls").join(format!("{example}.json"));
	let committed_raw = fs::read_to_string(&committed_path)
		.unwrap_or_else(|e| panic!("failed to read {}: {e}", committed_path.display()));
	let committed: Value = serde_json::from_str(&committed_raw)
		.unwrap_or_else(|e| panic!("failed to parse {}: {e}", committed_path.display()));
	let generated: Value = serde_json::to_value(example_program_idl(example))
		.unwrap_or_else(|e| panic!("failed to serialize generated IDL for {example}: {e}"));

	assert_eq!(
		generated,
		committed,
		"generated IDL for {example} diverged from {}",
		committed_path.display()
	);
}

#[test]
fn counter_program_idl() {
	let idl = example_program_idl("counter_program");
	insta::assert_json_snapshot!("counter_program", idl);
}

#[test]
fn escrow_program_idl() {
	let idl = example_program_idl("escrow_program");
	insta::assert_json_snapshot!("escrow_program", idl);
}

#[test]
fn todo_program_idl() {
	let idl = example_program_idl("todo_program");
	insta::assert_json_snapshot!("todo_program", idl);
}

#[test]
fn transfer_sol_idl() {
	let idl = example_program_idl("transfer_sol");
	insta::assert_json_snapshot!("transfer_sol", idl);
}

#[test]
fn hello_solana_idl() {
	let idl = example_program_idl("hello_solana");
	insta::assert_json_snapshot!("hello_solana", idl);
}

#[test]
fn compact_accounts_idl_preserves_multiple_dynamic_tails() {
	let idl = serde_json::to_value(example_program_idl("compact_accounts"))
		.unwrap_or_else(|error| panic!("serialize compact_accounts IDL: {error}"));
	let fields = idl
		.pointer("/program/accounts/0/data/fields")
		.and_then(Value::as_array)
		.expect("compact account fields");
	assert_eq!(fields.len(), 8);
	let featured_entry = &fields[4];
	let title = &fields[5];
	let entries = &fields[6];
	let markers = &fields[7];

	assert_eq!(
		featured_entry.pointer("/name").and_then(Value::as_str),
		Some("featuredEntry"),
	);
	assert_eq!(
		featured_entry.pointer("/type/kind").and_then(Value::as_str),
		Some("optionTypeNode"),
	);
	assert_eq!(
		featured_entry
			.pointer("/type/fixed")
			.and_then(Value::as_bool),
		Some(true),
	);
	assert_eq!(
		title.pointer("/name").and_then(Value::as_str),
		Some("title")
	);
	assert_eq!(
		title.pointer("/type/type/kind").and_then(Value::as_str),
		Some("sizePrefixTypeNode"),
	);
	assert_eq!(
		title
			.pointer("/type/type/type/encoding")
			.and_then(Value::as_str),
		Some("utf8"),
	);
	assert_eq!(
		entries.pointer("/name").and_then(Value::as_str),
		Some("entries")
	);
	assert_eq!(
		entries.pointer("/type/count/kind").and_then(Value::as_str),
		Some("prefixedCountNode"),
	);
	assert_eq!(
		entries
			.pointer("/type/count/prefix/type/type/format")
			.and_then(Value::as_str),
		Some("u16"),
	);
	assert_eq!(
		entries.pointer("/type/item/format").and_then(Value::as_str),
		Some("u64"),
	);
	assert_eq!(
		markers.pointer("/name").and_then(Value::as_str),
		Some("markers")
	);
	assert_eq!(
		markers.pointer("/type/count/kind").and_then(Value::as_str),
		Some("prefixedCountNode"),
	);
	assert_eq!(
		markers.pointer("/type/item/format").and_then(Value::as_str),
		Some("u8"),
	);
}

#[test]
fn committed_example_idls_match_generated_output() {
	for example in [
		"compact_accounts",
		"counter_program",
		"escrow_program",
		"hello_solana",
		"todo_program",
		"transfer_sol",
	] {
		assert_matches_committed_idl(example);
	}
}

#[test]
fn every_example_is_a_complete_pina_project() {
	let examples_dir = workspace_root().join("examples");

	for example in EXAMPLES {
		let root = examples_dir.join(example);
		let project = Project::discover(&root)
			.unwrap_or_else(|error| panic!("discover {example} through pina.toml: {error}"));

		assert_eq!(
			project.root,
			root.canonicalize()
				.unwrap_or_else(|error| panic!("canonicalize {example} root: {error}"))
		);
		assert_eq!(project.package_name, *example);

		let surfpool_manifest = root.join("tests/surfpool/Cargo.toml");
		let surfpool_cargo = fs::read_to_string(&surfpool_manifest)
			.unwrap_or_else(|error| panic!("read {}: {error}", surfpool_manifest.display()));
		assert!(surfpool_cargo.contains("pina_test = { workspace = true }"));
		assert!(!surfpool_cargo.contains("pina_test = { path ="));
		assert!(root.join("tests/surfpool/src/lib.rs").is_file());
	}
}
