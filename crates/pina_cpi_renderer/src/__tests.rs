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
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::ProgramNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::RootNode;
use codama_nodes::TypeNode;
use codama_nodes::U8;
use codama_nodes::U64;
use codama_nodes::ValueNode;

use super::render::wire;
use super::render::wire::TypeIndex;
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
	render_instruction_page(page, &mut TypeIndex::default())
		.unwrap_or_else(|error| panic!("renders: {error}"))
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
fn migration_version_is_part_of_the_framework_owned_cpi_prefix() {
	let root = load_fixture_root("migrations_program");
	let update = root
		.program
		.instructions
		.iter()
		.find(|candidate| candidate.name.as_ref() == "update")
		.unwrap_or_else(|| panic!("fixture has no `update` instruction"));
	let content = render_fixture_instruction("migrations_program", "update");

	// The oracle must not share code with the renderer, so build the expected
	// prefix bytes straight from the fixture's constant discriminator nodes:
	// offset 0 carries the instruction discriminator and offset 1 the current
	// migration version, both little-endian u8.
	let mut bytes = Vec::new();
	for node in &update.discriminators {
		let DiscriminatorNode::Constant(node) = node else {
			panic!("fixture uses a non-constant discriminator");
		};
		let (TypeNode::Number(r#type), ValueNode::Number(value)) =
			(&*node.constant.r#type, &*node.constant.value)
		else {
			panic!("fixture discriminator is not a numeric constant");
		};
		assert_eq!(
			node.offset as usize,
			bytes.len(),
			"prefix must be contiguous"
		);
		assert_eq!(r#type.format, U8, "fixture prefix is u8");
		let Number::UnsignedInteger(byte) = value.number else {
			panic!("fixture discriminator value is not an unsigned integer");
		};
		bytes.push(u8::try_from(byte).unwrap_or_else(|error| panic!("byte fits u8: {error}")));
	}
	let discriminator_const = format!(
		"const UPDATE_DISCRIMINATOR: [u8; {}] = {:?};",
		bytes.len(),
		bytes
	);

	assert!(content.contains("pub const LEN: usize = 12;"));
	assert!(content.contains("data[..2].copy_from_slice(&UPDATE_DISCRIMINATOR);"));
	assert!(content.contains("data[2..10].copy_from_slice(&self.value.to_le_bytes());"));
	assert!(content.contains("data[10..12].copy_from_slice(&self.memo.to_le_bytes());"));
	assert!(content.contains(&discriminator_const));
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
fn scaffold_names_the_crate_after_the_package_name() {
	let root = load_fixture_root("vesting_program");

	let snake = RenderConfig {
		package_name: Some("compact_accounts_program".to_string()),
		..RenderConfig::default()
	};
	let snake_dir = unique_temp_dir("pina-cpi-renderer-name-snake");
	render_root_node(&root, &snake_dir, &snake).unwrap_or_else(|error| panic!("renders: {error}"));
	assert!(snake_dir.join("Cargo.toml").is_file());
	let manifest = fs::read_to_string(snake_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("reads: {error}"));
	assert!(manifest.contains("name = \"compact_accounts_program_cpi\""));
	fs::remove_dir_all(&snake_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));

	let kebab = RenderConfig {
		package_name: Some("my-kebab-program".to_string()),
		..RenderConfig::default()
	};
	let kebab_dir = unique_temp_dir("pina-cpi-renderer-name-kebab");
	render_root_node(&root, &kebab_dir, &kebab).unwrap_or_else(|error| panic!("renders: {error}"));
	let manifest = fs::read_to_string(kebab_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("reads: {error}"));
	assert!(manifest.contains("name = \"my-kebab-program-cpi\""));
	fs::remove_dir_all(&kebab_dir).unwrap_or_else(|error| panic!("cleans up: {error}"));
}

#[test]
fn scaffold_falls_back_to_the_snake_cased_program_name() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-name-fallback");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));

	let manifest = fs::read_to_string(crate_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("reads: {error}"));
	// The IDL program name `vestingProgram` snake-cases to `vesting_program`.
	assert!(manifest.contains("name = \"vesting_program_cpi\""));

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
fn generation_modes_create_update_and_overwrite_destinations() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-modes");
	let create = RenderConfig {
		mode: RenderMode::Create,
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &create)
		.unwrap_or_else(|error| panic!("create failed: {error}"));
	let error = render_root_node(&root, &crate_dir, &create)
		.expect_err("create must reject a nonempty destination");
	assert!(matches!(error, RenderError::InvalidGenerationState { .. }));

	fs::write(crate_dir.join("Cargo.toml"), "# consumer manifest\n")
		.unwrap_or_else(|error| panic!("manifest edit failed: {error}"));
	let update = RenderConfig {
		mode: RenderMode::Update,
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &update)
		.unwrap_or_else(|error| panic!("update failed: {error}"));
	assert_eq!(
		fs::read_to_string(crate_dir.join("Cargo.toml"))
			.unwrap_or_else(|error| panic!("manifest read failed: {error}")),
		"# consumer manifest\n"
	);

	fs::write(crate_dir.join("sentinel.txt"), "remove")
		.unwrap_or_else(|error| panic!("sentinel write failed: {error}"));
	let overwrite = RenderConfig {
		mode: RenderMode::Overwrite,
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &overwrite)
		.unwrap_or_else(|error| panic!("overwrite failed: {error}"));
	assert!(!crate_dir.join("sentinel.txt").exists());
	assert!(crate_dir.join("Cargo.toml").is_file());

	fs::remove_dir_all(crate_dir)
		.unwrap_or_else(|error| panic!("mode fixture cleanup failed: {error}"));
}

#[test]
fn source_only_generation_does_not_create_a_crate_scaffold() {
	let root = load_fixture_root("vesting_program");
	let crate_dir = unique_temp_dir("pina-cpi-renderer-source-only");
	let update = RenderConfig {
		mode: RenderMode::Update,
		..RenderConfig::default()
	};
	let error = render_root_node(&root, &crate_dir, &update)
		.expect_err("update must reject a missing destination");
	assert!(matches!(error, RenderError::InvalidGenerationState { .. }));

	let source_only = RenderConfig {
		scaffold: false,
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &source_only)
		.unwrap_or_else(|error| panic!("source-only render failed: {error}"));
	assert!(crate_dir.join("src/generated/mod.rs").is_file());
	assert!(!crate_dir.join("Cargo.toml").exists());
	assert!(!crate_dir.join("src/lib.rs").exists());

	fs::remove_dir_all(crate_dir)
		.unwrap_or_else(|error| panic!("source-only fixture cleanup failed: {error}"));
}

#[test]
fn generation_mode_labels_cover_the_public_policy() {
	assert_eq!(RenderMode::Auto.as_str(), "automatically generate");
	assert_eq!(RenderMode::Create.as_str(), "create");
	assert_eq!(RenderMode::Update.as_str(), "update");
	assert_eq!(RenderMode::Overwrite.as_str(), "overwrite");
}

#[cfg(unix)]
#[test]
fn generation_modes_reject_unsafe_destination_trees_and_unreadable_paths() {
	use std::os::unix::fs::PermissionsExt;
	use std::os::unix::fs::symlink;

	let root = load_fixture_root("vesting_program");
	let output = unique_temp_dir("pina-cpi-render-mode-safety");
	fs::create_dir_all(&output)
		.unwrap_or_else(|error| panic!("failed to create safety fixture: {error}"));

	let file = output.join("file");
	fs::write(&file, "blocked")
		.unwrap_or_else(|error| panic!("failed to create file target: {error}"));
	assert!(matches!(
		render_root_node(&root, &file, &RenderConfig::default()),
		Err(RenderError::UnsafeOutputPath { .. })
	));

	let real = output.join("real");
	let linked = output.join("linked");
	fs::create_dir_all(&real)
		.unwrap_or_else(|error| panic!("failed to create symlink target: {error}"));
	symlink(&real, &linked)
		.unwrap_or_else(|error| panic!("failed to create destination symlink: {error}"));
	assert!(matches!(
		render_root_node(&root, &linked, &RenderConfig::default()),
		Err(RenderError::UnsafeOutputPath { .. })
	));

	let git_tree = output.join("git-tree");
	fs::create_dir_all(git_tree.join(".git"))
		.unwrap_or_else(|error| panic!("failed to create Git marker: {error}"));
	assert!(matches!(
		remove_crate_dir(&git_tree),
		Err(RenderError::UnsafeOutputPath { .. })
	));

	let linked_tree = output.join("linked-tree");
	fs::create_dir_all(&linked_tree)
		.unwrap_or_else(|error| panic!("failed to create linked tree: {error}"));
	symlink(&real, linked_tree.join("child"))
		.unwrap_or_else(|error| panic!("failed to create nested symlink: {error}"));
	assert!(matches!(
		remove_crate_dir(&linked_tree),
		Err(RenderError::UnsafeOutputPath { .. })
	));
	assert!(remove_crate_dir(&output.join("missing")).is_ok());

	let blocked_parent = output.join("blocked-parent");
	fs::create_dir_all(&blocked_parent)
		.unwrap_or_else(|error| panic!("failed to create blocked parent: {error}"));
	fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o000))
		.unwrap_or_else(|error| panic!("failed to block parent: {error}"));
	let unreadable_metadata = render_root_node(
		&root,
		&blocked_parent.join("child"),
		&RenderConfig::default(),
	);
	fs::set_permissions(&blocked_parent, fs::Permissions::from_mode(0o700))
		.unwrap_or_else(|error| panic!("failed to restore parent: {error}"));
	assert!(matches!(
		unreadable_metadata,
		Err(RenderError::ReadFile { .. })
	));

	let blocked_dir = output.join("blocked-dir");
	fs::create_dir_all(&blocked_dir)
		.unwrap_or_else(|error| panic!("failed to create blocked directory: {error}"));
	fs::set_permissions(&blocked_dir, fs::Permissions::from_mode(0o000))
		.unwrap_or_else(|error| panic!("failed to block directory: {error}"));
	let unreadable_entries = render_root_node(&root, &blocked_dir, &RenderConfig::default());
	fs::set_permissions(&blocked_dir, fs::Permissions::from_mode(0o700))
		.unwrap_or_else(|error| panic!("failed to restore directory: {error}"));
	assert!(matches!(
		unreadable_entries,
		Err(RenderError::ReadFile { .. })
	));

	fs::remove_dir_all(output)
		.unwrap_or_else(|error| panic!("failed to clean safety fixture: {error}"));
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
	let page = render_instruction_page(&root.program.instructions[0], &mut TypeIndex::default())
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
	let page = render_instruction_page(&root.program.instructions[0], &mut TypeIndex::default())
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
fn refuses_the_omitted_optional_account_strategy() {
	let mut root = load_fixture_root("vesting_program");
	root.program.instructions[0].accounts[0].is_optional = Some(true);
	root.program.instructions[0].optional_account_strategy =
		Some(codama_nodes::OptionalAccountStrategy::Omitted);
	let error = render_program_to_files(&root, &RenderConfig::default())
		.err()
		.expect("the omitted strategy cannot build a shortened account list");

	assert!(
		error
			.to_string()
			.contains("omitted optional-account strategy")
	);

	// Even trailing optionals are refused: a fixed-size handle set cannot drop
	// them, and filling the slots would send accounts the callee does not
	// expect under this strategy.
	let mut root = load_fixture_root("vesting_program");
	let mut witness = InstructionAccountNode::new("witness", false, IsSigner::False);
	witness.is_optional = Some(true);
	root.program.instructions[0].accounts.push(witness);
	root.program.instructions[0].optional_account_strategy =
		Some(codama_nodes::OptionalAccountStrategy::Omitted);
	let error = render_program_to_files(&root, &RenderConfig::default())
		.err()
		.expect("trailing omitted optionals are refused as well");
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
	let error = render_program_to_files(&root, &RenderConfig::default()).expect_err("refuses");
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
	let page = render_instruction_page(initialize, &mut TypeIndex::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));

	// The wire buffer also reserves the migration version byte that the
	// fixture IDL carries between the discriminator and the payload.
	assert!(page.contains(&format!("[0u8; {}", 2 + 8 * wire_args - 7)));
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
	let error = render_program_to_files(&RootNode::new(program), &RenderConfig::default())
		.expect_err("refuses");
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));

	assert!(page.contains("pub sponsor: &'argument Address,"));
	assert!(page.contains("/// Instruction argument `sponsor`."));
	assert!(page.contains("/// Address credited as the sponsor."));
	assert!(page.contains("pub active: bool,"));
	assert!(page.contains("pub stake: u64,"));
	assert!(page.contains("pub member: &'account AccountView,"));
	assert!(page.contains("CpiHandle::writable_signer(self.member)?"));
	assert!(page.contains("CpiHandle::readonly_signer(self.authority)"));
	assert!(page.contains("pub ix: EnrollIx<'argument>,"));
	assert!(page.contains("pub fn invoke(&self, program: &ProgramAccount<'_>)"));
	assert!(page.contains("pub fn invoke_signed("));
	assert!(page.contains("context.invoke_signed(&data, signers)"));
	assert!(page.contains("[0u8; 42]"));
}

#[test]
fn renders_signed_number_arguments() {
	let program = program_node(
		"registry",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"adjust",
			numeric_discriminator(3),
			vec![InstructionAccountNode::new("member", true, true)],
			vec![
				InstructionArgumentNode::new("delta8", NumberTypeNode::le(NumberFormat::I8)),
				InstructionArgumentNode::new("delta16", NumberTypeNode::le(NumberFormat::I16)),
				InstructionArgumentNode::new("delta32", NumberTypeNode::le(NumberFormat::I32)),
				InstructionArgumentNode::new("delta64", NumberTypeNode::le(NumberFormat::I64)),
				InstructionArgumentNode::new("delta128", NumberTypeNode::le(NumberFormat::I128)),
			],
		)],
	);
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));

	for (field, rust_type) in [
		("delta8", "i8"),
		("delta16", "i16"),
		("delta32", "i32"),
		("delta64", "i64"),
		("delta128", "i128"),
	] {
		assert!(page.contains(&format!("pub {field}: {rust_type},")));
	}

	// Signed arguments use the same two's-complement little-endian write as the
	// unsigned formats, so the offsets depend only on the declared widths.
	assert!(page.contains("data[1..2].copy_from_slice(&self.delta8.to_le_bytes());"));
	assert!(page.contains("data[2..4].copy_from_slice(&self.delta16.to_le_bytes());"));
	assert!(page.contains("data[4..8].copy_from_slice(&self.delta32.to_le_bytes());"));
	assert!(page.contains("data[8..16].copy_from_slice(&self.delta64.to_le_bytes());"));
	assert!(page.contains("data[16..32].copy_from_slice(&self.delta128.to_le_bytes());"));
	assert!(page.contains("[0u8; 32]"));
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
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
	assert!(render_program_to_files(&RootNode::new(rejected), &RenderConfig::default()).is_err());
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
		.unwrap_or_else(|error| panic!("renders address-only instruction: {error}"));

	assert!(page.contains("pub struct SetOwner<'argument> {"));
	assert!(page.contains("pub ix: SetOwnerIx<'argument>,"));
	assert!(page.contains("impl<'argument> SetOwnerIx<'argument>"));
	assert!(page.contains("impl<'argument> SetOwner<'argument>"));
}

#[test]
fn shares_one_argument_lifetime_for_addresses_and_pinapod_strings() {
	let string = codama_nodes::SizePrefixTypeNode::<TypeNode>::new(
		codama_nodes::StringTypeNode::utf8(),
		NumberTypeNode::le(U8),
	);
	let string = codama_nodes::FixedSizeTypeNode::new(string, 33);
	let program = program_node(
		"registry",
		"11111111111111111111111111111111",
		vec![instruction_node(
			"setProfile",
			numeric_discriminator(9),
			vec![],
			vec![
				InstructionArgumentNode::new("owner", PublicKeyTypeNode {}),
				InstructionArgumentNode::new("name", string),
			],
		)],
	);
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
		.unwrap_or_else(|error| panic!("renders address and String arguments: {error}"));

	assert!(page.contains("pub struct SetProfile<'argument> {"));
	assert!(page.contains("pub owner: &'argument Address,"));
	assert!(page.contains("pub name: &'argument str,"));
	assert!(page.contains("pub ix: SetProfileIx<'argument>,"));
	assert!(page.contains("impl<'argument> SetProfileIx<'argument>"));
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
	let page = render_instruction_page(&program.instructions[0], &mut TypeIndex::default())
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
	let files = render_program_to_files(&root, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("renders: {error}"));
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
	let files = render_program_to_files(&empty, &RenderConfig::default())
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

/// Loads a checked-in foreign IDL fixture.
fn foreign_fixture_root(name: &str) -> RootNode {
	let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
		.join("fixtures")
		.join(format!("{name}.json"));
	read_root_node(&fixture_path)
		.unwrap_or_else(|error| panic!("failed to load foreign fixture `{name}`: {error}"))
}

/// Every checked-in foreign IDL, as `(fixture name, instruction count)`.
const FOREIGN_FIXTURES: &[(&str, usize)] = &[
	("switchboard_on_demand", 4),
	("metaplex_token_metadata", 58),
	("meteora_dlmm", 76),
	("squads_v4_multisig", 36),
];

#[test]
fn renders_every_foreign_idl_fixture() {
	for (name, expected_instructions) in FOREIGN_FIXTURES {
		let root = foreign_fixture_root(name);
		assert_eq!(
			root.program.instructions.len(),
			*expected_instructions,
			"fixture `{name}` changed shape"
		);

		let config = RenderConfig {
			skip_unsupported_instructions: true,
			..RenderConfig::default()
		};
		let files = render_program_to_files(&root, &config)
			.unwrap_or_else(|error| panic!("fixture `{name}` failed to render: {error}"));

		// Every rendered instruction gets a page, and the root module wires the
		// result. Skipped instructions are recorded in `instructions/mod.rs`.
		assert!(files.contains_key(Path::new("mod.rs")), "fixture `{name}`");
		assert!(
			files.contains_key(Path::new("programs.rs")),
			"fixture `{name}`"
		);
		let instructions_mod = files
			.get(Path::new("instructions/mod.rs"))
			.unwrap_or_else(|| panic!("fixture `{name}` has no instructions module"));
		for instruction in &root.program.instructions {
			let page = format!("instructions/{}.rs", snake(instruction.name.as_ref()));
			let rendered = files.contains_key(Path::new(&page));
			let skipped =
				instructions_mod.contains(&format!("// Skipped `{}`", instruction.name.as_ref()));
			assert!(
				rendered || skipped,
				"fixture `{name}` neither rendered nor skipped `{}`",
				instruction.name.as_ref()
			);
		}
	}
}

#[test]
fn generates_compilable_sources_for_every_foreign_idl() {
	for (name, _) in FOREIGN_FIXTURES {
		let root = foreign_fixture_root(name);
		// Real-world IDLs carry instructions whose account lists this renderer
		// cannot express yet (Anchor's `omitted` strategy), so the fixture gate
		// renders with skips allowed and the recorded skips stay visible.
		let config = RenderConfig {
			skip_unsupported_instructions: true,
			..RenderConfig::default()
		};
		let files = render_program_to_files(&root, &config)
			.unwrap_or_else(|error| panic!("fixture `{name}` failed to render: {error}"));

		// `render_program_to_files` already parses each page with `syn`, so a
		// pass here means every generated file is syntactically valid Rust.
		assert!(!files.is_empty(), "fixture `{name}` produced no files");
	}
}

/// Switchboard is the flagship case from the hardening issue, and the
/// hand-written `pina-rs/lootbox` crate is its capability benchmark. These
/// values are pinned to that crate and to the deployed program's Anchor IDL.
#[test]
fn switchboard_matches_the_hand_written_reference_crate() {
	let root = foreign_fixture_root("switchboard_on_demand");
	let files = render_program_to_files(&root, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("switchboard should render: {error}"));

	// Discriminators pinned to the hand-written crate's unit tests.
	let expected = [
		(
			"randomness_init",
			"[9, 9, 204, 33, 50, 116, 113, 15]",
			13,
			16,
		),
		(
			"randomness_commit",
			"[52, 170, 152, 201, 179, 133, 242, 141]",
			5,
			8,
		),
		(
			"randomness_reveal",
			"[197, 181, 187, 10, 30, 58, 20, 73]",
			12,
			105,
		),
		(
			"randomness_close",
			"[146, 101, 14, 74, 225, 246, 0, 156]",
			10,
			8,
		),
	];

	for (instruction, discriminator, accounts, encoded_len) in expected {
		let page = format!("instructions/{instruction}.rs");
		let source = files
			.get(Path::new(&page))
			.unwrap_or_else(|| panic!("switchboard is missing `{page}`"));

		assert!(
			source.contains(discriminator),
			"`{instruction}` discriminator drifted: expected {discriminator}"
		);
		assert_eq!(
			source.matches("CpiHandle::").count(),
			accounts,
			"`{instruction}` account list drifted"
		);
		assert!(
			source.contains(&format!("pub const LEN: usize = {encoded_len};")),
			"`{instruction}` encoded length drifted: expected {encoded_len}"
		);
	}
}

#[test]
fn documents_every_rejected_foreign_argument_shape() {
	// The issue requires a deliberate, recorded decision for each type the
	// renderer refuses. These are the shapes it must never accept silently.
	let cases = [
		(
			"shortU16",
			NumberTypeNode::le(NumberFormat::ShortU16),
			"variable-length prefix",
		),
		(
			"f32",
			NumberTypeNode::le(NumberFormat::F32),
			"floating-point",
		),
		(
			"big-endian u16",
			NumberTypeNode::be(NumberFormat::U16),
			"little-endian",
		),
	];

	for (label, node, expected_reason) in cases {
		let error = wire::plan(
			&node.into(),
			&mut TypeIndex::default(),
			"documented rejection",
		)
		.expect_err(&format!("`{label}` must be rejected"));
		assert!(
			error.to_string().contains(expected_reason),
			"`{label}` rejection must explain `{expected_reason}`, got: {error}"
		);
	}
}

#[test]
fn renders_read_only_account_parsers_for_foreign_idls() {
	for (name, _) in FOREIGN_FIXTURES {
		let root = foreign_fixture_root(name);
		let config = RenderConfig {
			skip_unsupported_instructions: true,
			..RenderConfig::default()
		};
		let files = render_program_to_files(&root, &config)
			.unwrap_or_else(|error| panic!("fixture `{name}` failed to render: {error}"));

		for account in &root.program.accounts {
			let module = snake(account.name.as_ref());
			let page = format!("accounts/{module}.rs");
			let source = files
				.get(Path::new(&page))
				.unwrap_or_else(|| panic!("fixture `{name}` is missing `{page}`"));

			// Every account gets its declared layout plus a discriminator guard,
			// whether or not a parser could be generated for it.
			assert!(
				source.contains("pub fn matches("),
				"`{page}` has no discriminator guard"
			);
			assert!(
				source.contains("pub const LEN: usize")
					|| source.contains("pub const MAX_LEN: usize"),
				"`{page}` declares no encoded size"
			);
		}
	}
}

/// The Switchboard randomness account is the parser the hardening issue calls
/// out: `pina-rs/lootbox` hand-wrote 408 bytes of offset arithmetic for it.
#[test]
fn switchboard_account_parser_matches_the_hand_written_offsets() {
	let root = foreign_fixture_root("switchboard_on_demand");
	let files = render_program_to_files(&root, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("switchboard should render: {error}"));
	let source = files
		.get(Path::new("accounts/randomness_account_data.rs"))
		.unwrap_or_else(|| panic!("switchboard has no randomness account parser"));

	// Size pinned to `RandomnessAccountData` in `switchboard-on-demand` 0.13.0.
	assert!(source.contains("pub const LEN: usize = 408;"));
	// Account discriminator pinned to the deployed program, and to the
	// hand-written crate's `RANDOMNESS_ACCOUNT_DISCRIMINATOR`.
	assert!(source.contains("[10, 66, 229, 135, 220, 239, 217, 114]"));
	assert!(source.contains("pub fn parse(data: &[u8])"));
	assert!(source.contains("pub fn matches(data: &[u8]) -> bool"));

	// Each field the hand-written parser reads must be present.
	for field in [
		"authority",
		"queue",
		"seed_slothash",
		"seed_slot",
		"oracle",
		"reveal_slot",
		"value",
	] {
		assert!(
			source.contains(&format!("pub {field}:")),
			"missing `{field}`"
		);
	}
}

/// Pina's own compact accounts use `preOffset`/`postOffset` nodes and a
/// discriminator at a non-zero offset. Neither has a statically known position,
/// so the account ships its layout without a parser instead of failing to
/// render — a regression that previously broke `test:idl` for every example.
#[test]
fn renders_examples_with_runtime_relative_account_layouts() {
	for name in ["compact_accounts_program", "migrations_program"] {
		let root = load_fixture_root(name);
		let files = render_program_to_files(&root, &RenderConfig::default())
			.unwrap_or_else(|error| panic!("`{name}` should render: {error}"));

		for account in &root.program.accounts {
			let page = format!("accounts/{}.rs", snake(account.name.as_ref()));
			let source = files
				.get(Path::new(&page))
				.unwrap_or_else(|| panic!("`{name}` is missing `{page}`"));

			// The layout ships either way; a parser is optional.
			assert!(
				source.contains("pub struct "),
				"`{page}` has no layout struct"
			);
			if !source.contains("pub fn parse(") {
				assert!(
					source.contains("PARSER_UNSUPPORTED"),
					"`{page}` omits its parser without recording why"
				);
			}
		}
	}
}

/// Boolean account fields previously decoded as `let value = value;`, which
/// referenced itself and failed to compile. Every committed example with a
/// boolean account field must produce a parser that reads it from the buffer.
#[test]
fn account_parsers_read_boolean_fields_from_the_buffer() {
	for name in [
		"role_registry_program",
		"validation_program",
		"escrow_program",
	] {
		let root = load_fixture_root(name);
		let files = render_program_to_files(&root, &RenderConfig::default())
			.unwrap_or_else(|error| panic!("`{name}` should render: {error}"));

		for (path, source) in &files {
			if !path.starts_with("accounts") || !source.contains("pub fn parse(") {
				continue;
			}
			// A self-referential binding is the failure this guards against.
			assert!(
				!source.contains("let value = value;"),
				"`{name}` {path:?} decodes a field into itself"
			);
			for line in source.lines() {
				let trimmed = line.trim();
				if let Some(rest) = trimmed.strip_prefix("let ") {
					if let Some((name, value)) = rest.split_once(" = ") {
						let name = name.trim_end_matches(':').trim();
						assert_ne!(
							name,
							value.trim_end_matches(';').trim(),
							"`{name}` {path:?} binds `{name}` to itself"
						);
					}
				}
			}
		}
	}
}

mod account_planning {
	use codama_nodes::StructFieldTypeNode;

	use super::render::accounts;
	use super::*;

	fn account_node(name: &str, fields: Vec<StructFieldTypeNode>) -> codama_nodes::AccountNode {
		codama_nodes::AccountNode {
			name: name.into(),
			data: codama_nodes::NestedTypeNode::Value(codama_nodes::StructTypeNode::new(fields)),
			discriminators: Vec::new().into(),
			docs: Vec::new().into(),
			pda: None,
			size: None,
		}
	}

	fn field(name: &str, r#type: TypeNode) -> StructFieldTypeNode {
		StructFieldTypeNode::new(name, r#type)
	}

	/// A discriminator field: present in the layout, but baked into the program
	/// with an omitted default the account parser can read its bytes from.
	fn omitted_field(name: &str, default: ValueNode) -> StructFieldTypeNode {
		let mut node = StructFieldTypeNode::new(name, NumberTypeNode::le(NumberFormat::U64));
		node.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		node.default_value = Box::new(Some(default));
		node
	}

	fn discriminated_account(
		name: &str,
		fields: Vec<StructFieldTypeNode>,
		discriminators: Vec<DiscriminatorNode>,
	) -> codama_nodes::AccountNode {
		codama_nodes::AccountNode {
			discriminators: discriminators.into(),
			..account_node(name, fields)
		}
	}

	fn amount_field() -> StructFieldTypeNode {
		field(
			"amount",
			TypeNode::Number(NumberTypeNode::le(NumberFormat::U64)),
		)
	}

	fn field_discriminator(name: &str, offset: u64) -> DiscriminatorNode {
		DiscriminatorNode::Field(codama_nodes::FieldDiscriminatorNode::new(name, offset))
	}

	#[test]
	fn uses_the_declared_field_name_or_a_positional_fallback() {
		let named = field("authority", TypeNode::PublicKey(PublicKeyTypeNode::new()));
		let unnamed = field("", TypeNode::Number(NumberTypeNode::le(NumberFormat::U8)));
		assert_eq!(accounts::field_name(&named, 0), "authority");
		assert_eq!(accounts::field_name(&unnamed, 1), "field_1");
	}

	#[test]
	fn reads_field_discriminator_bytes_from_every_literal_shape() {
		let mut types = TypeIndex::default();
		let account = discriminated_account(
			"tagged",
			vec![
				omitted_field(
					"tag",
					ValueNode::Bytes(BytesValueNode::base16("a1b2c3d4e5f6a7b8")),
				),
				amount_field(),
			],
			vec![field_discriminator("tag", 0)],
		);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("bytes discriminator should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("TAGGED_DISCRIMINATOR"));
		assert!(page.contains("161, 178, 195, 212, 229, 246, 167, 184"));

		let account = discriminated_account(
			"numbered",
			vec![
				omitted_field(
					"tag",
					ValueNode::Number(NumberValueNode::new(Number::UnsignedInteger(1))),
				),
				amount_field(),
			],
			vec![field_discriminator("tag", 0)],
		);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("number discriminator should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("[1, 0, 0, 0, 0, 0, 0, 0]"));

		let account = discriminated_account(
			"signed",
			vec![
				omitted_field(
					"tag",
					ValueNode::Number(NumberValueNode::new(Number::SignedInteger(-1))),
				),
				amount_field(),
			],
			vec![field_discriminator("tag", 0)],
		);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("signed discriminator should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("[255, 255, 255, 255, 255, 255, 255, 255]"));
	}

	#[test]
	fn rejects_unusable_field_discriminators() {
		// A field discriminator naming a field the layout does not carry.
		let account = discriminated_account(
			"ghosted",
			vec![amount_field()],
			vec![field_discriminator("tag", 0)],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a missing discriminator field must be rejected");
		assert!(error.to_string().contains("not present"));

		// A discriminator field without an omitted default has no bytes to read.
		let account = discriminated_account(
			"optional",
			vec![amount_field()],
			vec![field_discriminator("amount", 0)],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a non-omitted discriminator field must be rejected");
		assert!(error.to_string().contains("no omitted default value"));

		// A float literal has no integer byte representation.
		let account = discriminated_account(
			"floated",
			vec![
				omitted_field(
					"tag",
					ValueNode::Number(NumberValueNode::new(Number::Float(1.5))),
				),
				amount_field(),
			],
			vec![field_discriminator("tag", 0)],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a float discriminator must be rejected");
		assert!(error.to_string().contains("float"));

		// A boolean default is not a byte or number literal.
		let account = discriminated_account(
			"flagged",
			vec![
				omitted_field(
					"tag",
					ValueNode::String(codama_nodes::StringValueNode::new("nope")),
				),
				amount_field(),
			],
			vec![field_discriminator("tag", 0)],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a non-literal discriminator must be rejected");
		assert!(error.to_string().contains("byte or number literal"));
	}

	#[test]
	fn constant_discriminators_accept_bytes_and_numbers_only() {
		let mut types = TypeIndex::default();
		let number_constant = || {
			DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::le(NumberFormat::U64)),
					ValueNode::Number(NumberValueNode::new(Number::UnsignedInteger(7))),
				),
				0,
			))
		};

		let account =
			discriminated_account("stamped", vec![amount_field()], vec![number_constant()]);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("constant number discriminator should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("STAMPED_DISCRIMINATOR"));
		assert!(page.contains("[7, 0, 0, 0, 0, 0, 0, 0]"));

		let account = discriminated_account(
			"branded",
			vec![amount_field()],
			vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Bytes(codama_nodes::BytesTypeNode::new()),
					ValueNode::Bytes(BytesValueNode::base16("a1b2c3d4")),
				),
				0,
			))],
		);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("constant bytes discriminator should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("BRANDED_DISCRIMINATOR"));
		assert!(page.contains("161, 178, 195, 212"));

		let account = discriminated_account(
			"seedy",
			vec![amount_field()],
			vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::String(codama_nodes::StringTypeNode::utf8()),
					ValueNode::String(codama_nodes::StringValueNode::new("anchor")),
				),
				0,
			))],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a string constant discriminator must be rejected");
		assert!(error.to_string().contains("byte or number literals"));

		let account = discriminated_account(
			"floated",
			vec![amount_field()],
			vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::le(NumberFormat::F64)),
					ValueNode::Number(NumberValueNode::new(Number::Float(1.5))),
				),
				0,
			))],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a float constant discriminator must be rejected");
		assert!(error.to_string().contains("float"));
	}

	#[test]
	fn withholds_the_parser_for_a_nonzero_field_discriminator_offset() {
		// A field discriminator that does not continue from offset zero cannot
		// gate a parser, because no byte prefix identifies the account.
		let account = discriminated_account(
			"late",
			vec![amount_field()],
			vec![field_discriminator("amount", 8)],
		);
		let planned = accounts::plan_account(&account, &mut TypeIndex::default())
			.unwrap_or_else(|error| panic!("the layout should still plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(!page.contains("LATE_DISCRIMINATOR"));
		assert!(page.contains("!data.is_empty()"));
	}

	#[test]
	fn combines_adjacent_constant_discriminators_into_one_prefix() {
		// Migration-aware IDLs tag an account with several adjacent one-byte
		// constants (the program discriminator plus a migration version), which
		// together form the guard's prefix.
		let number_constant = |offset: u64, value: u64| {
			DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::le(NumberFormat::U8)),
					ValueNode::Number(NumberValueNode::new(Number::UnsignedInteger(value))),
				),
				offset,
			))
		};
		let account = discriminated_account(
			"migrated",
			vec![amount_field()],
			vec![number_constant(0, 1), number_constant(1, 0)],
		);
		let planned = accounts::plan_account(&account, &mut TypeIndex::default())
			.unwrap_or_else(|error| panic!("adjacent constants should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("MIGRATED_DISCRIMINATOR: [u8; 2] = [1, 0]"));
		assert!(page.contains("data.len() >= 2"));

		let account = discriminated_account(
			"skipped",
			vec![amount_field()],
			vec![number_constant(0, 1), number_constant(2, 0)],
		);
		let planned = accounts::plan_account(&account, &mut TypeIndex::default())
			.unwrap_or_else(|error| panic!("a gapped prefix should still plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(!page.contains("SKIPPED_DISCRIMINATOR"));
	}

	#[test]
	fn narrows_discriminator_bytes_to_the_declared_width() {
		let mut types = TypeIndex::default();
		let constant = |format: NumberFormat, value: Number| {
			DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::le(format)),
					ValueNode::Number(NumberValueNode::new(value)),
				),
				0,
			))
		};

		// Each declared width produces exactly that many bytes.
		for (format, expected) in [
			(NumberFormat::U8, "[9]"),
			(NumberFormat::U16, "[9, 0]"),
			(NumberFormat::U32, "[9, 0, 0, 0]"),
			(NumberFormat::U64, "[9, 0, 0, 0, 0, 0, 0, 0]"),
		] {
			let account = discriminated_account(
				"sized",
				vec![amount_field()],
				vec![constant(format, Number::UnsignedInteger(9))],
			);
			let planned = accounts::plan_account(&account, &mut types)
				.unwrap_or_else(|error| panic!("{format:?} should plan: {error}"));
			let page = accounts::render_planned_account(&planned);
			assert!(
				page.contains(expected),
				"{format:?} rendered:
{page}"
			);
		}

		// A signed literal keeps its two's-complement pattern.
		let account = discriminated_account(
			"negative",
			vec![amount_field()],
			vec![constant(NumberFormat::U8, Number::SignedInteger(-1))],
		);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("a signed literal should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("[255]"));
	}

	#[test]
	fn rejects_discriminators_that_do_not_fit_their_declared_width() {
		let constant = |format: NumberFormat, value: Number| {
			DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::le(format)),
					ValueNode::Number(NumberValueNode::new(value)),
				),
				0,
			))
		};

		// 256 does not fit a u8, and a signed -1 does not fit a u8 either once
		// narrowed; both are reported instead of silently truncating.
		for value in [Number::UnsignedInteger(256), Number::SignedInteger(-129)] {
			let account = discriminated_account(
				"overflow",
				vec![amount_field()],
				vec![constant(NumberFormat::U8, value.clone())],
			);
			let error = accounts::plan_account(&account, &mut TypeIndex::default())
				.err()
				.expect("an out-of-range discriminator must be rejected");
			assert!(error.to_string().contains("does not fit"));
		}

		// Big-endian and unsupported widths are rejected with their reasons.
		let account = discriminated_account(
			"big_endian",
			vec![amount_field()],
			vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::be(NumberFormat::U8)),
					ValueNode::Number(NumberValueNode::new(Number::UnsignedInteger(1))),
				),
				0,
			))],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("big-endian discriminators must be rejected");
		assert!(error.to_string().contains("little-endian"));

		let account = discriminated_account(
			"wide",
			vec![amount_field()],
			vec![constant(NumberFormat::U128, Number::UnsignedInteger(1))],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("u128 discriminators must be rejected");
		assert!(error.to_string().contains("at most 8 bytes"));
	}

	#[test]
	fn rejects_number_literals_whose_declared_type_is_not_a_number() {
		// A malformed IDL can pair a number literal with a non-number type; the
		// mismatch is reported rather than assumed to be eight bytes.
		let mut field =
			StructFieldTypeNode::new("tag", TypeNode::PublicKey(PublicKeyTypeNode::new()));
		field.default_value_strategy = Some(codama_nodes::DefaultValueStrategy::Omitted);
		field.default_value = Box::new(Some(ValueNode::Number(NumberValueNode::new(
			Number::UnsignedInteger(1),
		))));
		let account = discriminated_account(
			"mismatched",
			vec![field, amount_field()],
			vec![field_discriminator("tag", 0)],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a number literal without a number type must be rejected");
		assert!(error.to_string().contains("must declare a number type"));

		// The same mismatch on a constant discriminator.
		let account = discriminated_account(
			"constant_mismatch",
			vec![amount_field()],
			vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::PublicKey(PublicKeyTypeNode::new()),
					ValueNode::Number(NumberValueNode::new(Number::UnsignedInteger(1))),
				),
				0,
			))],
		);
		let error = accounts::plan_account(&account, &mut TypeIndex::default())
			.err()
			.expect("a number constant without a number type must be rejected");
		assert!(error.to_string().contains("must declare a number type"));
	}

	#[test]
	fn only_a_whole_address_identifier_pulls_in_the_import() {
		// A type whose name merely contains `Address` must not trigger the
		// import, or the generated page fails a `-D warnings` build.
		assert!(accounts::test_mentions_address("Address"));
		assert!(accounts::test_mentions_address("Option<Address>"));
		assert!(accounts::test_mentions_address("&'argument Address"));
		assert!(!accounts::test_mentions_address("AddressBook"));
		assert!(!accounts::test_mentions_address("[AddressLike; 4]"));
		assert!(!accounts::test_mentions_address("u64"));
	}

	#[test]
	fn withholds_the_parser_for_a_size_discriminator() {
		// A size discriminator is metadata, not bytes this client can check.
		let account = discriminated_account(
			"sized",
			vec![amount_field()],
			vec![DiscriminatorNode::Size(
				codama_nodes::SizeDiscriminatorNode::new(8),
			)],
		);
		let planned = accounts::plan_account(&account, &mut TypeIndex::default())
			.unwrap_or_else(|error| panic!("the layout should still plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(!page.contains("SIZED_DISCRIMINATOR"));
	}

	#[test]
	fn withholds_the_parser_for_a_nonzero_constant_discriminator() {
		// A constant discriminator at a non-zero offset cannot gate a parser.
		let account = discriminated_account(
			"shifted",
			vec![amount_field()],
			vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(
					TypeNode::Number(NumberTypeNode::le(NumberFormat::U64)),
					ValueNode::Number(NumberValueNode::new(Number::UnsignedInteger(7))),
				),
				8,
			))],
		);
		let planned = accounts::plan_account(&account, &mut TypeIndex::default())
			.unwrap_or_else(|error| panic!("the layout should still plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(!page.contains("SHIFTED_DISCRIMINATOR"));
	}

	#[test]
	fn renders_accounts_without_a_discriminator_or_parser() {
		let root = load_fixture_root("vesting_program");
		// The vesting IDL has no accounts module; exercise the renderer through
		// a minimal synthetic account with an unsupported field.
		let mut types = TypeIndex::default();
		let account = account_node(
			"opaque",
			vec![field(
				"payload",
				TypeNode::String(codama_nodes::StringTypeNode::utf8()),
			)],
		);
		let planned = accounts::plan_account(&account, &mut types)
			.unwrap_or_else(|error| panic!("should plan: {error}"));
		let page = accounts::render_planned_account(&planned);
		assert!(page.contains("pub struct Opaque"));
		// A bare string has no fixed size, so the account reports a ceiling.
		assert!(page.contains("pub const MAX_LEN") || page.contains("pub const LEN"));
	}
}

mod foreign_fixture_accounts {
	use super::render::accounts;
	use super::*;

	/// Accounts whose fields cannot all be located still ship their layout,
	/// with the reason recorded instead of a parser that would misread.
	///
	/// Metaplex also carries one instruction using the `omitted`
	/// optional-account strategy, which the fixed-size handle set cannot
	/// express, so the render skips it and records the reason.
	#[test]
	fn renders_parser_unsupported_accounts_with_their_reason() {
		let root = foreign_fixture_root("metaplex_token_metadata");
		let config = RenderConfig {
			skip_unsupported_instructions: true,
			..RenderConfig::default()
		};
		let files = render_program_to_files(&root, &config)
			.unwrap_or_else(|error| panic!("render: {error}"));

		let page = files
			.get(Path::new("accounts/collection_authority_record.rs"))
			.unwrap_or_else(|| panic!("missing collection authority record"));
		assert!(page.contains("PARSER_UNSUPPORTED"));
		assert!(page.contains("pub struct CollectionAuthorityRecord"));

		let instructions_mod = files
			.get(Path::new("instructions/mod.rs"))
			.unwrap_or_else(|| panic!("missing instructions mod"));
		assert!(instructions_mod.contains("Skipped `deprecatedMintNewEdition"));
		assert!(instructions_mod.contains("omitted optional-account strategy"));
		assert!(!instructions_mod.contains("pub(crate) mod r#deprecated_mint_new_edition"));
	}

	/// Without the skip flag an unsupported instruction fails the whole render.
	#[test]
	fn fails_closed_on_the_omitted_optional_account_strategy() {
		let root = foreign_fixture_root("metaplex_token_metadata");
		let error = render_program_to_files(&root, &RenderConfig::default())
			.err()
			.expect("the omitted strategy must fail a default render");
		assert!(
			error
				.to_string()
				.contains("omitted optional-account strategy")
		);
	}

	/// A discriminated account with only fixed-width fields gets a complete
	/// parser and a LEN that matches the on-chain layout.
	#[test]
	fn switchboard_parser_has_the_full_layout() {
		let root = foreign_fixture_root("switchboard_on_demand");
		let files = render_program_to_files(&root, &RenderConfig::default())
			.unwrap_or_else(|error| panic!("render: {error}"));
		let page = files
			.get(Path::new("accounts/randomness_account_data.rs"))
			.unwrap_or_else(|| panic!("missing randomness account"));
		assert!(page.contains("pub const LEN: usize = 408;"));
		assert!(page.contains("pub fn parse(data: &[u8])"));
	}
}

mod defined_type_pages {
	use codama_nodes::DefinedTypeNode;
	use codama_nodes::StructFieldTypeNode;

	use super::*;

	#[test]
	fn rejects_defined_types_that_are_not_structs_or_enums() {
		let mut types = TypeIndex::default();
		let defined = DefinedTypeNode::new("plain", NumberTypeNode::le(NumberFormat::U64));
		let error = render::types::render_type_page(&defined, &mut types)
			.expect_err("scalar defined types must be rejected");
		assert!(error.to_string().contains("become Rust types"));
	}

	#[test]
	fn propagates_field_planning_errors_from_struct_pages() {
		let mut types = TypeIndex::default();
		let defined = DefinedTypeNode::new(
			"broken",
			codama_nodes::StructTypeNode::new(vec![StructFieldTypeNode::new(
				"bare",
				codama_nodes::StringTypeNode::utf8(),
			)]),
		);
		let error = render::types::render_type_page(&defined, &mut types)
			.expect_err("bare strings must be rejected");
		assert!(error.to_string().contains("length prefix"));
	}

	#[test]
	fn renders_enum_pages_with_wide_discriminants() {
		for format in [
			NumberFormat::U8,
			NumberFormat::U16,
			NumberFormat::U32,
			NumberFormat::U64,
		] {
			let mut types = TypeIndex::default();
			let defined = DefinedTypeNode::new(
				"sized",
				codama_nodes::EnumTypeNode {
					variants: vec![codama_nodes::EnumEmptyVariantTypeNode::new("only").into()],
					size: NumberTypeNode::le(format).into(),
				},
			);
			let page = render::types::render_type_page(&defined, &mut types)
				.unwrap_or_else(|error| panic!("{format:?} should render: {error}"));
			assert!(page.contains("pub enum Sized"));
		}

		let mut types = TypeIndex::default();
		let defined = DefinedTypeNode::new(
			"bad",
			codama_nodes::EnumTypeNode {
				variants: vec![codama_nodes::EnumEmptyVariantTypeNode::new("only").into()],
				size: NumberTypeNode::le(NumberFormat::U128).into(),
			},
		);
		let error = render::types::render_type_page(&defined, &mut types)
			.expect_err("u128 discriminants must be rejected");
		assert!(error.to_string().contains("discriminants must be"));
	}
}
