use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codama_nodes::ArrayTypeNode;
use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesEncoding;
use codama_nodes::BytesValueNode;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::Docs;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionArgumentNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::ProgramNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::RootNode;
use codama_nodes::U8;
use codama_nodes::U64;

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
		.unwrap_or_else(|error| panic!("failed to load fixture `{name}`: {error}"))
}

fn render_fixture_instruction(name: &str, instruction: &str) -> String {
	let root = load_fixture_root(name);
	let page = root
		.program
		.instructions
		.iter()
		.find(|candidate| candidate.name.as_ref() == instruction)
		.unwrap_or_else(|| panic!("fixture `{name}` has no `{instruction}` instruction"));
	render_instruction_page(page).unwrap_or_else(|error| panic!("renders: {error}"))
}

fn program_node(name: &str, public_key: &str, instructions: Vec<InstructionNode>) -> ProgramNode {
	ProgramNode {
		name: name.into(),
		public_key: public_key.to_string(),
		version: "0.0.0".to_string(),
		origin: None,
		docs: Docs::default(),
		accounts: vec![],
		instructions,
		defined_types: vec![],
		pdas: vec![],
		events: vec![],
		errors: vec![],
		constants: vec![],
	}
}

fn instruction_node(
	name: &str,
	discriminator: DiscriminatorNode,
	accounts: Vec<InstructionAccountNode>,
	arguments: Vec<InstructionArgumentNode>,
) -> InstructionNode {
	InstructionNode {
		name: name.into(),
		docs: Docs::default(),
		optional_account_strategy: None,
		accounts,
		arguments,
		extra_arguments: vec![],
		remaining_accounts: vec![],
		byte_deltas: vec![],
		discriminators: vec![discriminator],
		status: None,
		sub_instructions: vec![],
		provides: vec![],
		display: None,
		plugins: vec![],
	}
}

fn numeric_discriminator(value: u8) -> DiscriminatorNode {
	DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
		ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(value)),
		0,
	))
}

#[test]
fn vesting_initialize_instruction_snapshot() {
	insta::assert_snapshot!(render_fixture_instruction("vesting_program", "initialize"));
}

#[test]
fn vesting_claim_instruction_snapshot() {
	insta::assert_snapshot!(render_fixture_instruction("vesting_program", "claim"));
}

#[test]
fn vesting_cancel_instruction_snapshot() {
	insta::assert_snapshot!(render_fixture_instruction("vesting_program", "cancel"));
}

#[test]
fn renders_vesting_fixture_to_disk() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-vesting");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));

	let generated = crate_dir.join("src/generated");
	assert!(generated.join("mod.rs").is_file());
	assert!(generated.join("programs.rs").is_file());
	assert!(generated.join("instructions/mod.rs").is_file());
	assert!(generated.join("instructions/initialize.rs").is_file());
	assert!(crate_dir.join("Cargo.toml").is_file());
	assert!(crate_dir.join("src/lib.rs").is_file());

	let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs"))
		.unwrap_or_else(|error| panic!("reads: {error}"));
	assert_eq!(lib_rs, "pub mod generated;\npub use generated::*;\n");

	// Re-rendering must succeed against the managed generated directory.
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("re-renders: {error}"));

	fs::remove_dir_all(&crate_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));
}

#[test]
fn scaffold_never_overwrites_consumer_files() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-pinned");
	fs::create_dir_all(crate_dir.join("src")).unwrap_or_else(|error| panic!("creates: {error}"));
	fs::write(
		crate_dir.join("Cargo.toml"),
		"[package]\nname = \"pinned\"\n",
	)
	.unwrap_or_else(|error| panic!("writes: {error}"));
	fs::write(crate_dir.join("src/lib.rs"), "// pinned\n")
		.unwrap_or_else(|error| panic!("writes: {error}"));

	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));

	let cargo_toml = fs::read_to_string(crate_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("reads: {error}"));
	assert!(cargo_toml.starts_with("[package]\nname = \"pinned\""));
	let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs"))
		.unwrap_or_else(|error| panic!("reads: {error}"));
	assert_eq!(lib_rs, "// pinned\n");

	fs::remove_dir_all(&crate_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));
}

#[test]
fn refuses_a_directory_not_created_by_this_renderer() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-foreign");
	let generated = crate_dir.join("src/generated");
	fs::create_dir_all(&generated).unwrap_or_else(|error| panic!("creates: {error}"));
	fs::write(generated.join("mod.rs"), "// not ours\n")
		.unwrap_or_else(|error| panic!("writes: {error}"));

	let error = render_root_node(&root, &crate_dir, &RenderConfig::default()).expect_err("refuses");
	assert!(error.to_string().contains("not created by this renderer"));

	fs::remove_dir_all(&crate_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));
}

#[test]
fn refuses_optional_accounts() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].accounts[0].is_optional = Some(true);
	let error = render_program_to_files(&root).expect_err("refuses");
	assert!(
		error
			.to_string()
			.contains("optional accounts are not supported")
	);
}

#[test]
fn refuses_optional_signers() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].accounts[0].is_signer = IsSigner::Either;
	let error = render_program_to_files(&root).expect_err("refuses");
	assert!(
		error
			.to_string()
			.contains("optional signers are not supported")
	);
}

#[test]
fn refuses_optional_arguments() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].arguments[1].default_value_strategy =
		Some(codama_nodes::DefaultValueStrategy::Optional);
	let error = render_program_to_files(&root).expect_err("refuses");
	assert!(
		error
			.to_string()
			.contains("optional arguments are not supported")
	);
}

#[test]
fn renders_non_omitted_arguments_only() {
	let mut root = load_fixture_root("vesting_program");
	let initialize = &mut root.program.instructions[0];
	let wire_args = initialize
		.arguments
		.iter()
		.filter(|argument| {
			!matches!(
				argument.default_value_strategy,
				Some(codama_nodes::DefaultValueStrategy::Omitted)
			)
		})
		.count();
	let page =
		render_instruction_page(initialize).unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains(&format!("[0u8; {}", 1 + 8 * wire_args - 7)));
	assert!(!page.contains("discriminator:"));
}

#[test]
fn refuses_non_constant_discriminators() {
	let program = program_node(
		"broken",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"tap",
			DiscriminatorNode::Field(codama_nodes::FieldDiscriminatorNode::new("kind", 0)),
			vec![],
			vec![],
		)],
	);
	let error = render_program_to_files(&RootNode::new(program)).expect_err("refuses");
	assert!(error.to_string().contains("missing required discriminator"));
}

#[test]
fn renders_byte_array_arguments() {
	let program = program_node(
		"hasher",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"seal",
			numeric_discriminator(3),
			vec![],
			vec![InstructionArgumentNode::new(
				"digest",
				ArrayTypeNode::fixed(NumberTypeNode::le(U8), 32),
			)],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains("pub digest: [u8; 32]"));
	assert!(page.contains("const SEAL_DISCRIMINATOR: [u8; 1] = [3];"));
	assert!(page.contains("pub struct Seal<'a>"));
}

#[test]
fn renders_base16_discriminators() {
	let program = program_node(
		"sealed",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"open",
			DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					ArrayTypeNode::fixed(NumberTypeNode::le(U8), 4),
					BytesValueNode::new(BytesEncoding::Base16, "0xdeadbeef"),
				),
				0,
			)),
			vec![],
			vec![],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains("const OPEN_DISCRIMINATOR: [u8; 4] = [222, 173, 190, 239];"));
	assert!(page.contains("[0u8; 4]"));
	assert!(page.contains("pub struct Open<'a>"));
}

#[test]
fn renders_public_key_bool_and_number_arguments() {
	let program = program_node(
		"registry",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"enroll",
			numeric_discriminator(7),
			vec![InstructionAccountNode::new("member", true, true)],
			vec![
				InstructionArgumentNode::new("sponsor", PublicKeyTypeNode {}),
				InstructionArgumentNode::new("active", BooleanTypeNode::default()),
				InstructionArgumentNode::new("stake", NumberTypeNode::le(U64)),
			],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains("pub sponsor: Address,"));
	assert!(page.contains("pub active: bool,"));
	assert!(page.contains("pub stake: u64,"));
	assert!(page.contains("pub member: &'a AccountView,"));
	assert!(page.contains("InstructionAccount::new(self.member.address(), true, true)"));
	assert!(page.contains("[0u8; 42]"));
}

#[test]
fn renders_fixed_size_byte_arguments() {
	let program = program_node(
		"todo",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"stamp",
			numeric_discriminator(5),
			vec![],
			vec![InstructionArgumentNode::new(
				"digest",
				codama_nodes::FixedSizeTypeNode::new(codama_nodes::BytesTypeNode {}, 32),
			)],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains("pub digest: [u8; 32]"));
	assert!(page.contains("[0u8; 33]"));
	assert!(page.contains("data[1..33].copy_from_slice(&self.digest);"));
}

#[test]
fn renders_program_id_constants() {
	let root = RootNode::new(program_node(
		"registry",
		"Bp6AJD3QQ64kZVfc1YnhP7GN5UBYEHsDXpGUc1xzg4op",
		vec![instruction_node(
			"tap",
			numeric_discriminator(0),
			vec![],
			vec![],
		)],
	));
	let files = render_program_to_files(&root).unwrap_or_else(|error| panic!("renders: {error}"));
	let programs_rs = &files[&PathBuf::from("programs.rs")];

	assert!(programs_rs.contains("pub const REGISTRY_ID: Address ="));
	assert!(programs_rs.contains("Bp6AJD3QQ64kZVfc1YnhP7GN5UBYEHsDXpGUc1xzg4op"));
}
