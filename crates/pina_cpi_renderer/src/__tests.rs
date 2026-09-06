use std::collections::BTreeMap;
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
	assert_eq!(
		lib_rs,
		"#![no_std]\n\npub mod generated;\npub use generated::*;\n"
	);

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
fn scaffold_imports_a_custom_generated_folder() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-custom-folder");
	let config = RenderConfig {
		generated_folder: PathBuf::from("src/custom/cpi"),
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &config)
		.unwrap_or_else(|error| panic!("renders custom folder: {error}"));

	let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs"))
		.unwrap_or_else(|error| panic!("reads scaffold: {error}"));
	assert!(lib_rs.contains("#[path = \"custom/cpi/mod.rs\"]"));
	assert!(crate_dir.join("src/custom/cpi/mod.rs").is_file());

	fs::remove_dir_all(&crate_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));
}

#[test]
fn refuses_a_directory_not_created_by_this_renderer() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-foreign");
	let generated = crate_dir.join("src/generated");
	fs::create_dir_all(&generated).unwrap_or_else(|error| panic!("creates: {error}"));
	let crate_handle =
		open_crate_dir(&crate_dir).unwrap_or_else(|error| panic!("opens crate directory: {error}"));
	validate_existing_generated_dir(&crate_handle, &crate_dir, Path::new("src/generated"), true)
		.unwrap_or_else(|error| panic!("accepts an empty managed directory: {error}"));
	validate_existing_generated_dir(&crate_handle, &crate_dir, Path::new("src/generated"), false)
		.unwrap_or_else(|error| panic!("accepts an ordinary empty directory: {error}"));
	fs::write(generated.join("foreign.rs"), "// not ours\n")
		.unwrap_or_else(|error| panic!("writes: {error}"));

	let error = render_root_node(&root, &crate_dir, &RenderConfig::default()).expect_err("refuses");
	assert!(error.to_string().contains("not created by this renderer"));

	fs::remove_dir_all(&generated).unwrap_or_else(|error| panic!("resets: {error}"));
	fs::create_dir_all(&generated).unwrap_or_else(|error| panic!("recreates: {error}"));
	fs::write(generated.join("mod.rs"), "// not ours\n")
		.unwrap_or_else(|error| panic!("writes: {error}"));

	let error = render_root_node(&root, &crate_dir, &RenderConfig::default()).expect_err("refuses");
	assert!(error.to_string().contains("not created by this renderer"));

	fs::remove_dir_all(&crate_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));
}

#[test]
fn renders_program_id_optional_accounts() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].accounts[0].is_optional = Some(true);
	root.program.instructions[0].optional_account_strategy =
		Some(codama_nodes::OptionalAccountStrategy::ProgramId);
	let page = render_instruction_page(&root.program.instructions[0])
		.unwrap_or_else(|error| panic!("renders optional account: {error}"));

	assert!(page.contains("pub admin: Option<&'account AccountView>,"));
	assert!(page.contains("Some(account) => CpiHandle::writable_signer(account)?"));
	assert!(page.contains("None => CpiHandle::readonly(program.account())"));
}

#[test]
fn renders_runtime_signer_selection() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].accounts[0].is_signer = IsSigner::Either;
	let mut witness = InstructionAccountNode::new("witness", false, IsSigner::Either);
	witness.is_optional = Some(true);
	witness.docs = vec!["Optional witness account.".to_string()].into();
	root.program.instructions[0].accounts.push(witness);
	root.program.instructions[0].optional_account_strategy =
		Some(codama_nodes::OptionalAccountStrategy::ProgramId);
	let page = render_instruction_page(&root.program.instructions[0])
		.unwrap_or_else(|error| panic!("renders runtime signer: {error}"));

	assert!(page.contains("pub admin: (&'account AccountView, bool),"));
	assert!(page.contains("(account, true) => CpiHandle::writable_signer(account)?"));
	assert!(page.contains("(account, false) => CpiHandle::writable(account)?"));
	assert!(page.contains("pub witness: Option<(&'account AccountView, bool)>,"));
	assert!(page.contains("/// Optional witness account."));
	assert!(page.contains("Some((account, true)) => CpiHandle::readonly_signer(account)"));
	assert!(page.contains("Some((account, false)) => CpiHandle::readonly(account)"));
}

#[test]
fn refuses_omitted_optional_accounts() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].accounts[0].is_optional = Some(true);
	root.program.instructions[0].optional_account_strategy =
		Some(codama_nodes::OptionalAccountStrategy::Omitted);
	let error = render_program_to_files(&root).expect_err("refuses omitted optional accounts");

	assert!(
		error
			.to_string()
			.contains("omitted optional-account strategy")
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
fn refuses_field_discriminators_without_a_matching_argument() {
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
	assert!(error.to_string().contains("has no matching argument"));
}

#[test]
fn renders_anchor_field_discriminators() {
	let mut discriminator = InstructionArgumentNode::new(
		"discriminator",
		codama_nodes::FixedSizeTypeNode::new(codama_nodes::BytesTypeNode {}, 8),
	);
	discriminator.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
	discriminator.default_value = Box::new(Some(
		BytesValueNode::new(BytesEncoding::Base16, "afaf6d1f0d989bed").into(),
	));
	let program = program_node(
		"anchorCounter",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"initialize",
			DiscriminatorNode::Field(codama_nodes::FieldDiscriminatorNode::new(
				"discriminator",
				0,
			)),
			vec![],
			vec![discriminator],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains(
		"const INITIALIZE_DISCRIMINATOR: [u8; 8] = [175, 175, 109, 31, 13, 152, 155, 237];"
	));
	assert!(!page.contains("pub discriminator:"));
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
	assert!(page.contains("pub struct Seal {"));
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
	assert!(page.contains("pub struct Open {"));
}

#[test]
fn renders_public_key_bool_and_number_arguments() {
	let mut sponsor = InstructionArgumentNode::new("sponsor", PublicKeyTypeNode {});
	sponsor.docs = vec!["Address credited as the sponsor.".to_string()].into();
	let program = program_node(
		"registry",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"enroll",
			numeric_discriminator(7),
			vec![
				InstructionAccountNode::new("member", true, true),
				InstructionAccountNode::new("authority", false, true),
			],
			vec![
				sponsor,
				InstructionArgumentNode::new("active", BooleanTypeNode::default()),
				InstructionArgumentNode::new("stake", NumberTypeNode::le(U64)),
			],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains("pub sponsor: &'address Address,"));
	assert!(page.contains("/// Instruction argument `sponsor`."));
	assert!(page.contains("/// Address credited as the sponsor."));
	assert!(page.contains("pub active: bool,"));
	assert!(page.contains("pub stake: u64,"));
	assert!(page.contains("pub member: &'account AccountView,"));
	assert!(page.contains("CpiHandle::writable_signer(self.member)?"));
	assert!(page.contains("CpiHandle::readonly_signer(self.authority)"));
	assert!(page.contains("pub instruction: EnrollInstruction<'address>,"));
	assert!(page.contains("pub fn invoke(&self, program: &ProgramAccount<'_>)"));
	assert!(page.contains("pub fn invoke_signed("));
	assert!(page.contains("context.invoke_signed(&data, signers)"));
	assert!(page.contains("[0u8; 42]"));
}

#[test]
fn escapes_keyword_fields_and_rejects_unescapable_identifiers() {
	let program = program_node(
		"keywords",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"setType",
			numeric_discriminator(1),
			vec![InstructionAccountNode::new("match", false, false)],
			vec![InstructionArgumentNode::new(
				"type",
				NumberTypeNode::le(U64),
			)],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("keyword fields should render: {error}"));
	assert!(page.contains("pub r#match: &'account AccountView,"));
	assert!(page.contains("pub r#type: u64,"));
	assert!(page.contains("self.r#match"));
	assert!(page.contains("self.r#type.to_le_bytes()"));

	let rejected = program_node(
		"keywords",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"setSelf",
			numeric_discriminator(2),
			vec![InstructionAccountNode::new("self", false, false)],
			vec![],
		)],
	);
	assert!(render_program_to_files(&RootNode::new(rejected)).is_err());
}

#[test]
fn renders_address_only_instruction_lifetimes() {
	let program = program_node(
		"registry",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"setOwner",
			numeric_discriminator(9),
			vec![],
			vec![InstructionArgumentNode::new("owner", PublicKeyTypeNode {})],
		)],
	);
	let page = render_instruction_page(&program.instructions[0])
		.unwrap_or_else(|error| panic!("renders address-only instruction: {error}"));

	assert!(page.contains("pub struct SetOwner<'address> {"));
	assert!(page.contains("pub instruction: SetOwnerInstruction<'address>,"));
	assert!(page.contains("impl<'address> SetOwnerInstruction<'address>"));
	assert!(page.contains("impl<'address> SetOwner<'address>"));
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
	let mut root = RootNode::new(program_node(
		"registry",
		"Bp6AJD3QQ64kZVfc1YnhP7GN5UBYEHsDXpGUc1xzg4op",
		vec![instruction_node(
			"tap",
			numeric_discriminator(0),
			vec![],
			vec![],
		)],
	));
	root.program.docs = vec!["Primary registry".to_string()].into();
	root.additional_programs.push(program_node(
		"helper",
		"11111111111111111111111111111111",
		vec![],
	));
	let files = render_program_to_files(&root).unwrap_or_else(|error| panic!("renders: {error}"));
	let programs_rs = &files[&PathBuf::from("programs.rs")];

	assert!(programs_rs.contains("pub const REGISTRY_ID: Address ="));
	assert!(programs_rs.contains("Bp6AJD3QQ64kZVfc1YnhP7GN5UBYEHsDXpGUc1xzg4op"));
	assert!(programs_rs.contains("/// Primary registry"));
	assert!(programs_rs.contains("pub const HELPER_ID: Address ="));
}

#[test]
fn public_entrypoints_cover_files_programs_and_parse_errors() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-entrypoints");
	render_idl_file(
		&repo_root().join("codama/idls/vesting_program.json"),
		&crate_dir,
		&RenderConfig::default(),
	)
	.unwrap_or_else(|error| panic!("IDL should render: {error}"));

	let program_dir = unique_temp_dir("pina-cpi-renderer-program");
	render_program(&root.program, &program_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("program should render: {error}"));

	let empty = RootNode::new(program_node(
		"empty",
		"11111111111111111111111111111111",
		vec![],
	));
	let files = render_program_to_files(&empty)
		.unwrap_or_else(|error| panic!("empty program should render: {error}"));
	assert!(!files.contains_key(Path::new("instructions/mod.rs")));
	let root_mod = &files[Path::new("mod.rs")];
	assert!(!root_mod.contains("mod instructions;"));
	assert!(!root_mod.contains("pub use instructions::*;"));

	let missing = unique_temp_dir("pina-cpi-renderer-missing");
	assert!(matches!(
		read_root_node(&missing),
		Err(RenderError::ReadFile { .. })
	));
	let malformed = unique_temp_dir("pina-cpi-renderer-malformed");
	fs::write(&malformed, "{").unwrap_or_else(|error| panic!("writes malformed IDL: {error}"));
	assert!(matches!(
		read_root_node(&malformed),
		Err(RenderError::ParseIdl { .. })
	));

	for path in [crate_dir, program_dir] {
		fs::remove_dir_all(path).unwrap_or_else(|error| panic!("cleans output: {error}"));
	}
	fs::remove_file(malformed).unwrap_or_else(|error| panic!("cleans malformed IDL: {error}"));
}

#[cfg(unix)]
#[test]
fn output_validation_rejects_unsafe_and_unreadable_paths() {
	use std::os::unix::fs::symlink;

	let crate_dir = unique_temp_dir("pina-cpi-renderer-paths");
	fs::create_dir_all(&crate_dir).unwrap_or_else(|error| panic!("creates temp dir: {error}"));

	for generated in [
		Path::new(""),
		Path::new("../generated"),
		Path::new("/generated"),
		Path::new("generated"),
		Path::new("src"),
	] {
		assert!(matches!(
			validate_generated_folder(generated),
			Err(RenderError::UnsafeOutputPath { .. })
		));
	}
	validate_generated_folder(Path::new("src/generated"))
		.unwrap_or_else(|error| panic!("default generated folder should be valid: {error}"));
	let crate_handle =
		open_crate_dir(&crate_dir).unwrap_or_else(|error| panic!("opens crate directory: {error}"));
	let path_blocker = crate_dir.join("path-blocker");
	fs::write(&path_blocker, "not a directory")
		.unwrap_or_else(|error| panic!("writes path blocker: {error}"));
	assert!(matches!(
		validate_generated_path(
			&crate_handle,
			&crate_dir,
			Path::new("path-blocker/generated")
		),
		Err(RenderError::ReadFile { .. })
	));

	let linked = crate_dir.join("linked");
	symlink(crate_dir.join("missing-target"), &linked)
		.unwrap_or_else(|error| panic!("creates symlink: {error}"));
	assert!(matches!(
		validate_generated_path(&crate_handle, &crate_dir, Path::new("linked/generated")),
		Err(RenderError::UnsafeOutputPath { .. })
	));

	let file = crate_dir.join("file");
	fs::write(&file, "not a directory").unwrap_or_else(|error| panic!("writes file: {error}"));
	assert!(matches!(
		validate_existing_generated_dir(&crate_handle, &crate_dir, Path::new("file"), false),
		Err(RenderError::UnsafeOutputPath { .. })
	));
	assert!(matches!(
		validate_existing_generated_dir(
			&crate_handle,
			&crate_dir,
			Path::new("path-blocker/generated"),
			false
		),
		Err(RenderError::ReadFile { .. })
	));
	assert!(matches!(
		validate_tree_has_no_symlinks(&crate_handle, &crate_dir, Path::new("file")),
		Err(RenderError::ReadFile { .. })
	));
	let tree = crate_dir.join("tree");
	fs::create_dir_all(&tree).unwrap_or_else(|error| panic!("creates tree: {error}"));
	let regular_tree = crate_dir.join("regular-tree/nested");
	fs::create_dir_all(&regular_tree)
		.unwrap_or_else(|error| panic!("creates regular tree: {error}"));
	fs::write(regular_tree.join("file.rs"), "source")
		.unwrap_or_else(|error| panic!("writes regular tree: {error}"));
	validate_tree_has_no_symlinks(&crate_handle, &crate_dir, Path::new("regular-tree"))
		.unwrap_or_else(|error| panic!("regular tree should validate: {error}"));
	symlink(crate_dir.join("missing-target"), tree.join("link"))
		.unwrap_or_else(|error| panic!("creates tree symlink: {error}"));
	assert!(matches!(
		validate_tree_has_no_symlinks(&crate_handle, &crate_dir, Path::new("tree")),
		Err(RenderError::UnsafeOutputPath { .. })
	));

	fs::remove_dir_all(crate_dir).unwrap_or_else(|error| panic!("cleans paths: {error}"));
}

#[test]
fn generated_source_and_io_error_helpers_preserve_context() {
	let mut files = BTreeMap::new();
	files.insert(PathBuf::from("invalid.rs"), "pub fn".to_string());
	assert!(matches!(
		validate_generated_sources(&files),
		Err(RenderError::InvalidGeneratedSource { .. })
	));
	assert!(matches!(
		read_file_error(Path::new("read"), std::io::Error::other("failure")),
		RenderError::ReadFile { .. }
	));
	assert!(matches!(
		write_file_error(Path::new("write"), std::io::Error::other("failure")),
		RenderError::WriteFile { .. }
	));
}

#[test]
fn scaffold_reports_each_filesystem_failure() {
	let temp = unique_temp_dir("pina-cpi-renderer-scaffold-errors");
	fs::create_dir_all(&temp).unwrap_or_else(|error| panic!("creates temp dir: {error}"));

	let blocked_crate = temp.join("blocked-crate");
	fs::write(&blocked_crate, "file").unwrap_or_else(|error| panic!("writes blocker: {error}"));
	assert!(matches!(
		open_crate_dir(&blocked_crate),
		Err(RenderError::WriteFile { .. })
	));

	let mut files = BTreeMap::new();
	files.insert(PathBuf::from("nested/file.rs"), "source".to_string());
	let blocked_base = temp.join("blocked-base");
	fs::write(&blocked_base, "file").unwrap_or_else(|error| panic!("writes base blocker: {error}"));
	assert!(matches!(
		open_crate_dir(&blocked_base),
		Err(RenderError::WriteFile { .. })
	));

	let blocked_write = temp.join("blocked-write");
	fs::create_dir_all(blocked_write.join("nested/file.rs"))
		.unwrap_or_else(|error| panic!("creates write blocker: {error}"));
	let blocked_handle = open_crate_dir(&blocked_write)
		.unwrap_or_else(|error| panic!("opens blocked output: {error}"));
	assert!(matches!(
		write_files(&blocked_handle, &blocked_write, Path::new(""), &files),
		Err(RenderError::WriteFile { .. })
	));
	assert!(matches!(
		missing_generated_dir(Path::new("generated"), std::io::Error::other("failure")),
		Err(RenderError::ReadFile { .. })
	));

	fs::remove_dir_all(temp).unwrap_or_else(|error| panic!("cleans scaffold errors: {error}"));
}

#[cfg(unix)]
#[test]
fn scaffold_and_generated_writes_refuse_symlink_targets() {
	use std::os::unix::fs::symlink;

	let root = load_fixture_root("vesting_program");
	let temp = unique_temp_dir("pina-cpi-renderer-symlink-sentinel");
	let crate_dir = temp.join("crate");
	let sentinel = temp.join("sentinel");
	fs::create_dir_all(crate_dir.join("src"))
		.unwrap_or_else(|error| panic!("creates crate: {error}"));
	fs::write(&sentinel, "preserve me").unwrap_or_else(|error| panic!("writes sentinel: {error}"));
	symlink(&sentinel, crate_dir.join("src/lib.rs"))
		.unwrap_or_else(|error| panic!("links scaffold: {error}"));

	assert!(matches!(
		render_root_node(&root, &crate_dir, &RenderConfig::default()),
		Err(RenderError::UnsafeOutputPath { .. })
	));
	assert_eq!(
		fs::read_to_string(&sentinel).unwrap_or_default(),
		"preserve me"
	);

	fs::remove_file(crate_dir.join("src/lib.rs"))
		.unwrap_or_else(|error| panic!("removes scaffold link: {error}"));
	symlink(&sentinel, crate_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("links manifest scaffold: {error}"));
	assert!(matches!(
		render_root_node(&root, &crate_dir, &RenderConfig::default()),
		Err(RenderError::UnsafeOutputPath { .. })
	));
	assert_eq!(
		fs::read_to_string(&sentinel).unwrap_or_default(),
		"preserve me"
	);
	fs::remove_file(crate_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("removes manifest scaffold link: {error}"));
	let config = RenderConfig {
		delete_folder_before_rendering: false,
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &config)
		.unwrap_or_else(|error| panic!("creates initial output: {error}"));
	let generated_file = crate_dir.join("src/generated/programs.rs");
	fs::remove_file(&generated_file)
		.unwrap_or_else(|error| panic!("removes generated file: {error}"));
	symlink(&sentinel, &generated_file)
		.unwrap_or_else(|error| panic!("links generated file: {error}"));
	assert!(matches!(
		render_root_node(&root, &crate_dir, &config),
		Err(RenderError::UnsafeOutputPath { .. })
	));
	assert_eq!(
		fs::read_to_string(&sentinel).unwrap_or_default(),
		"preserve me"
	);

	fs::remove_dir_all(temp).unwrap_or_else(|error| panic!("cleans symlink test: {error}"));
}
