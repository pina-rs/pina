use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codama_nodes::AccountNode;
use codama_nodes::BooleanTypeNode;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantPdaSeedNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::DefinedTypeNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::Docs;
use codama_nodes::Endian;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionNode;
use codama_nodes::InstructionOptionalAccountStrategy;
use codama_nodes::IsAccountSigner;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::PdaLinkNode;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::ProgramNode;
use codama_nodes::RootNode;
use codama_nodes::StringTypeNode;
use codama_nodes::StringValueNode;
use codama_nodes::StructFieldTypeNode;
use codama_nodes::StructTypeNode;
use codama_nodes::TypeNode;
use codama_nodes::U8;

use super::render::seeds::render_variable_seed_parameter;
use super::*;

fn unique_temp_dir(prefix: &str) -> PathBuf {
	let nanos = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_nanos();
	std::env::temp_dir().join(format!("{prefix}-{nanos}"))
}

fn repo_root() -> PathBuf {
	Path::new(env!("CARGO_MANIFEST_DIR"))
		.parent()
		.and_then(Path::parent)
		.unwrap_or_else(|| Path::new("."))
		.to_path_buf()
}

fn load_fixture_root(name: &str) -> RootNode {
	let fixture_path = repo_root().join("codama/idls").join(format!("{name}.json"));
	read_root_node(&fixture_path)
		.unwrap_or_else(|e| panic!("failed to load fixture {}: {e}", fixture_path.display()))
}

fn render_fixture_program(name: &str, prefix: &str) -> PathBuf {
	let root = load_fixture_root(name);
	let output_dir = unique_temp_dir(prefix);
	let crate_dir = output_dir.join(name);
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|e| panic!("render failed for `{name}`: {e}"));
	crate_dir
}

fn read_generated_file(crate_dir: &Path, path: &str) -> String {
	let generated_path = crate_dir.join("src/generated").join(path);
	fs::read_to_string(&generated_path).unwrap_or_else(|e| {
		panic!(
			"failed to read generated file {}: {e}",
			generated_path.display()
		)
	})
}

#[test]
fn renders_counter_account_with_pod_types() {
	let crate_dir = render_fixture_program("counter_program", "pina-codama-render-counter");
	let content = read_generated_file(&crate_dir, "accounts/counter_state.rs");

	insta::assert_snapshot!("counter_state_account_rs", content);
}

#[test]
fn renders_instruction_data_with_discriminator_prefix() {
	let crate_dir = render_fixture_program("todo_program", "pina-codama-render-todo");
	let content = read_generated_file(&crate_dir, "instructions/initialize.rs");

	insta::assert_snapshot!("todo_initialize_instruction_rs", content);
}

#[test]
fn renders_root_mod_with_unused_program_reexport_allowance() {
	let crate_dir = render_fixture_program("anchor_declare_id", "pina-codama-render-root-mod");
	let content = read_generated_file(&crate_dir, "mod.rs");

	insta::assert_snapshot!("root_mod_with_unused_program_reexport_allowance", content);
}

#[test]
fn renders_instruction_account_metas_using_self_fields() {
	let crate_dir =
		render_fixture_program("counter_program", "pina-codama-render-self-account-metas");
	let initialize_content = read_generated_file(&crate_dir, "instructions/initialize.rs");
	let increment_content = read_generated_file(&crate_dir, "instructions/increment.rs");

	insta::assert_snapshot!(
		"instruction_account_metas_using_self_fields",
		format!("{initialize_content}\n\n{increment_content}")
	);
}

#[test]
fn renders_pda_helpers_for_linked_account() {
	let program = ProgramNode {
		name: "exampleProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![AccountNode {
			name: "state".into(),
			size: None,
			docs: Docs::default(),
			data: StructTypeNode::new(vec![]).into(),
			pda: Some(PdaLinkNode::new("statePda")),
			discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(1u8)),
				0,
			))],
		}],
		instructions: vec![],
		defined_types: vec![],
		pdas: vec![PdaNode::new(
			"statePda",
			vec![PdaSeedNode::Constant(ConstantPdaSeedNode::new(
				StringTypeNode::utf8(),
				StringValueNode::new("state"),
			))],
		)],
		errors: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let root = RootNode::new(program);

	let output_dir = unique_temp_dir("pina-codama-render-pda");
	let crate_dir = output_dir.join("example_program");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|e| panic!("render failed: {e}"));

	let content = read_generated_file(&crate_dir, "accounts/state.rs");

	insta::assert_snapshot!("pda_helpers_for_linked_account", content);
}

#[test]
fn renders_optional_accounts_with_program_fallback_strategy() {
	let mut optional_signer = InstructionAccountNode::new("optionalSigner", false, false);
	optional_signer.is_optional = true;
	optional_signer.is_signer = IsAccountSigner::Either;

	let program = ProgramNode {
		name: "optionalProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![],
		instructions: vec![InstructionNode {
			name: "maybe".into(),
			docs: Docs::default(),
			optional_account_strategy: InstructionOptionalAccountStrategy::ProgramId,
			accounts: vec![optional_signer],
			arguments: vec![],
			extra_arguments: vec![],
			remaining_accounts: vec![],
			byte_deltas: vec![],
			discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(9u8)),
				0,
			))],
			status: None,
			sub_instructions: vec![],
		}],
		defined_types: vec![],
		pdas: vec![],
		errors: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let output_dir = unique_temp_dir("pina-codama-render-optional-fallback");
	let crate_dir = output_dir.join("optional_program");
	render_root_node(&RootNode::new(program), &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|e| panic!("render failed: {e}"));

	let content = read_generated_file(&crate_dir, "instructions/maybe.rs");
	insta::assert_snapshot!("optional_accounts_with_program_fallback_strategy", content);
}

#[test]
fn rejects_variable_size_strings() {
	let program = ProgramNode {
		name: "badProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![AccountNode {
			name: "state".into(),
			size: None,
			docs: Docs::default(),
			data: StructTypeNode::new(vec![StructFieldTypeNode::new(
				"memo",
				StringTypeNode::utf8(),
			)])
			.into(),
			pda: None,
			discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(1u8)),
				0,
			))],
		}],
		instructions: vec![],
		defined_types: vec![],
		pdas: vec![],
		errors: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};

	let result = render_root_node(
		&RootNode::new(program),
		&unique_temp_dir("pina-codama-render-bad"),
		&RenderConfig::default(),
	);

	let err = match result {
		Ok(()) => panic!("expected string type render to fail"),
		Err(err) => err,
	};

	insta::assert_snapshot!("rejects_variable_size_strings", err.to_string());
}

#[test]
fn rejects_big_endian_numbers() {
	let program = ProgramNode {
		name: "bigEndianProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![AccountNode {
			name: "state".into(),
			size: None,
			docs: Docs::default(),
			data: StructTypeNode::new(vec![StructFieldTypeNode::new(
				"count",
				NumberTypeNode {
					format: NumberFormat::U16,
					endian: Endian::Big,
				},
			)])
			.into(),
			pda: None,
			discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(1u8)),
				0,
			))],
		}],
		instructions: vec![],
		defined_types: vec![],
		pdas: vec![],
		errors: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};

	let err = render_root_node(
		&RootNode::new(program),
		&unique_temp_dir("pina-codama-render-big-endian"),
		&RenderConfig::default(),
	)
	.err()
	.unwrap_or_else(|| panic!("expected big-endian render to fail"));

	insta::assert_snapshot!("rejects_big_endian_numbers", err.to_string());
}

#[test]
fn renders_defined_type_aliases_with_pod_wrappers() {
	let program = ProgramNode {
		name: "aliasProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![],
		instructions: vec![],
		defined_types: vec![DefinedTypeNode {
			name: "counter".into(),
			docs: Docs::default(),
			r#type: TypeNode::Number(NumberTypeNode {
				format: NumberFormat::U64,
				endian: Endian::Little,
			}),
		}],
		pdas: vec![],
		errors: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let root = RootNode::new(program);

	let output_dir = unique_temp_dir("pina-codama-render-alias");
	let crate_dir = output_dir.join("alias_program");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|e| panic!("render failed: {e}"));

	let content = read_generated_file(&crate_dir, "types/counter.rs");
	insta::assert_snapshot!("defined_type_alias_counter_rs", content);
}

#[test]
fn rejects_missing_instruction_discriminators() {
	let program = ProgramNode {
		name: "missingIxDisc".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![],
		instructions: vec![InstructionNode {
			name: "doThing".into(),
			docs: Docs::default(),
			optional_account_strategy: InstructionOptionalAccountStrategy::ProgramId,
			accounts: vec![InstructionAccountNode::new("payer", true, true)],
			arguments: vec![],
			extra_arguments: vec![],
			remaining_accounts: vec![],
			byte_deltas: vec![],
			discriminators: vec![],
			status: None,
			sub_instructions: vec![],
		}],
		defined_types: vec![],
		pdas: vec![],
		errors: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let root = RootNode::new(program);

	let err = render_root_node(
		&root,
		&unique_temp_dir("pina-codama-render-missing-discriminator"),
		&RenderConfig::default(),
	)
	.err()
	.unwrap_or_else(|| panic!("expected render to fail"));

	insta::assert_snapshot!("rejects_missing_instruction_discriminators", err.to_string());
}

#[test]
fn writes_scaffold_with_bytemuck_dependency() {
	let root = load_fixture_root("hello_solana");
	let output_dir = unique_temp_dir("pina-codama-render-scaffold");
	let crate_dir = output_dir.join("hello_solana");

	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|e| panic!("render failed: {e}"));

	let cargo_toml_path = crate_dir.join("Cargo.toml");
	let cargo_toml = fs::read_to_string(&cargo_toml_path).unwrap_or_else(|e| {
		panic!(
			"failed to read generated cargo manifest {}: {e}",
			cargo_toml_path.display()
		)
	});

	insta::assert_snapshot!("writes_scaffold_with_bytemuck_dependency", cargo_toml);
}

#[test]
fn validates_boolean_encoding_for_pda_variable_seed() {
	let boolean_type = BooleanTypeNode {
		size: NumberTypeNode {
			format: NumberFormat::U16,
			endian: Endian::Little,
		}
		.into(),
	};
	let err = render_variable_seed_parameter(
		"seed",
		&TypeNode::Boolean(boolean_type),
		"pda boolean seed test",
	)
	.err()
	.unwrap_or_else(|| panic!("expected invalid boolean encoding error"));

	insta::assert_snapshot!("validates_boolean_encoding_for_pda_variable_seed", err.to_string());
}

#[test]
fn snapshots_escrow_take_instruction() {
	let crate_dir = render_fixture_program("escrow_program", "pina-codama-render-escrow");
	let content = read_generated_file(&crate_dir, "instructions/take.rs");
	insta::assert_snapshot!("escrow_take_instruction_rs", content);
}
