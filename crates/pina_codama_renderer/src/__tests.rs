use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codama_nodes::AccountNode;
use codama_nodes::AccountValueNode;
use codama_nodes::ArrayTypeNode;
use codama_nodes::BooleanTypeNode;
use codama_nodes::BytesEncoding;
use codama_nodes::BytesTypeNode;
use codama_nodes::ConstantDiscriminatorNode;
use codama_nodes::ConstantPdaSeedNode;
use codama_nodes::ConstantValueNode;
use codama_nodes::DefinedTypeLinkNode;
use codama_nodes::DefinedTypeNode;
use codama_nodes::DiscriminatorNode;
use codama_nodes::Docs;
use codama_nodes::Endianness;
use codama_nodes::EnumEmptyVariantTypeNode;
use codama_nodes::EnumTypeNode;
use codama_nodes::EnumVariantTypeNode;
use codama_nodes::FixedSizeTypeNode;
use codama_nodes::InstructionAccountNode;
use codama_nodes::InstructionInputValueNode;
use codama_nodes::InstructionNode;
use codama_nodes::IsSigner;
use codama_nodes::NumberFormat;
use codama_nodes::NumberTypeNode;
use codama_nodes::NumberValueNode;
use codama_nodes::OptionTypeNode;
use codama_nodes::OptionalAccountStrategy;
use codama_nodes::PdaLinkNode;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::PdaSeedValueNode;
use codama_nodes::PdaValueNode;
use codama_nodes::PostOffsetTypeNode;
use codama_nodes::ProgramNode;
use codama_nodes::PublicKeyTypeNode;
use codama_nodes::RootNode;
use codama_nodes::SizePrefixTypeNode;
use codama_nodes::StringTypeNode;
use codama_nodes::StringValueNode;
use codama_nodes::StructFieldTypeNode;
use codama_nodes::StructTypeNode;
use codama_nodes::TypeNode;
use codama_nodes::U8;
use codama_nodes::VariablePdaSeedNode;

use super::render::capacity::CompactCapacityIndex;
use super::render::capacity::compact_capacity_marker_name;
use super::render::seeds::render_variable_seed_parameter;
use super::render::types::render_type_for_compact_tail;
use super::render::types::render_type_for_pod;
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

fn compact_capacity_root(marker: Option<DefinedTypeNode>) -> RootNode {
	let tail = ArrayTypeNode::prefixed(
		NumberTypeNode::le(NumberFormat::U64),
		NumberTypeNode::le(NumberFormat::U16),
	);
	let account = AccountNode {
		name: "journal".into(),
		size: None,
		docs: Docs::default(),
		data: StructTypeNode::new(vec![
			StructFieldTypeNode::new("discriminator", NumberTypeNode::le(U8)),
			StructFieldTypeNode::new("entries", tail),
		])
		.into(),
		pda: None,
		discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
			ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(1u8)),
			0,
		))],
	};
	let marker = marker.into_iter().collect();

	RootNode::new(ProgramNode {
		name: "capacityProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![account],
		instructions: vec![],
		defined_types: marker,
		pdas: vec![],
		errors: vec![],
		events: vec![],
		constants: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	})
}

fn compact_capacity_marker(capacity: usize) -> DefinedTypeNode {
	DefinedTypeNode::new(
		compact_capacity_marker_name("journal", "entries"),
		FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), capacity),
	)
}

#[test]
fn renders_counter_account_with_pod_types() {
	let crate_dir = render_fixture_program("counter_program", "pina-codama-render-counter");
	let content = read_generated_file(&crate_dir, "accounts/counter_state.rs");

	insta::assert_snapshot!("counter_state_account_rs", content);
}

#[test]
fn consumes_machine_readable_capacity_and_omits_marker_artifacts() {
	let root = compact_capacity_root(Some(compact_capacity_marker(8)));
	let index = CompactCapacityIndex::read(&root.program)
		.unwrap_or_else(|error| panic!("capacity index failed: {error}"));
	assert_eq!(
		index
			.capacity("journal", "entries")
			.unwrap_or_else(|error| panic!("capacity lookup failed: {error}")),
		8,
	);

	let crate_dir = unique_temp_dir("pina-codama-capacity-marker");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("render failed: {error}"));
	let account = read_generated_file(&crate_dir, "accounts/journal.rs");
	let root_module = read_generated_file(&crate_dir, "mod.rs");

	assert!(account.contains("pub entries: pina::Vec<u64, 8>"));
	assert!(!root_module.contains("pub mod types;"));
	assert!(!crate_dir.join("src/generated/types").exists());
}

#[test]
fn rejects_missing_or_malformed_capacity_markers_instead_of_reading_docs() {
	let mut missing = compact_capacity_root(None);
	let codama_nodes::NestedTypeNode::Value(data) = &mut missing.program.accounts[0].data else {
		panic!("expected direct compact account struct");
	};
	data.fields[1].docs = vec!["Pina compact capacity: 999.".to_string()].into();
	let error = CompactCapacityIndex::read(&missing.program)
		.expect_err("documentation cannot supply compact capacity");
	assert!(error.to_string().contains("missing its capacity marker"));

	let name = compact_capacity_marker_name("journal", "entries");
	let malformed = DefinedTypeNode::new(name, NumberTypeNode::le(U8));
	let malformed = compact_capacity_root(Some(malformed));
	let error = CompactCapacityIndex::read(&malformed.program)
		.expect_err("malformed compact capacity marker must fail closed");
	assert!(error.to_string().contains("fixedSizeTypeNode"));

	let orphan = DefinedTypeNode::new(
		compact_capacity_marker_name("journal", "missing"),
		FixedSizeTypeNode::<TypeNode>::new(BytesTypeNode::new(), 8),
	);
	let mut orphan_root = compact_capacity_root(Some(compact_capacity_marker(8)));
	orphan_root.program.defined_types.push(orphan);
	let error = CompactCapacityIndex::read(&orphan_root.program)
		.expect_err("orphan compact capacity marker must fail closed");
	assert!(error.to_string().contains("does not resolve"));
}

#[test]
fn renders_compact_account_fixture_with_dynamic_helpers() {
	let crate_dir =
		render_fixture_program("compact_accounts_program", "pina-codama-render-compact");
	let content = read_generated_file(&crate_dir, "accounts/journal.rs");
	let manifest = fs::read_to_string(crate_dir.join("Cargo.toml"))
		.unwrap_or_else(|error| panic!("read compact client manifest: {error}"));

	assert!(manifest.contains("pina = { workspace = true, features = [\"compact\"] }"));
	assert!(content.contains("#[pinapod(compact)]"));
	assert!(content.contains("pub entries: pina::Vec<u64, 8>"));
	assert!(content.contains("pub markers: pina::PodVec<u8, 8, 8>"));
	assert!(content.contains("pub const HEADER_SIZE: usize"));
	assert!(content.contains("pub fn initialize(data: &mut [u8], patch: JournalPatch<'_>)"));
	assert!(content.contains("pub fn from_bytes(data: &[u8])"));
	assert!(!content.contains("JournalMut"));
	syn::parse_file(&content)
		.unwrap_or_else(|error| panic!("generated compact account is invalid Rust: {error}"));
}

#[test]
fn renders_every_compact_collection_prefix() {
	for (format, expected) in [
		(U8, "pina::PodVec<u64, 64, 1>"),
		(NumberFormat::U16, "pina::Vec<u64, 64>"),
		(NumberFormat::U32, "pina::PodVec<u64, 64, 4>"),
		(NumberFormat::U64, "pina::PodVec<u64, 64, 8>"),
	] {
		let tail = TypeNode::from(ArrayTypeNode::prefixed(
			NumberTypeNode::le(NumberFormat::U64),
			NumberTypeNode::le(format),
		));
		assert_eq!(
			render_type_for_compact_tail(&tail, 64, "State.values")
				.unwrap_or_else(|error| panic!("render failed: {error}")),
			expected
		);
	}
}

#[test]
fn renders_compact_tail_through_a_post_offset_wrapper() {
	let array = ArrayTypeNode::prefixed(
		NumberTypeNode::le(NumberFormat::U64),
		NumberTypeNode::le(NumberFormat::U16),
	);
	let tail = PostOffsetTypeNode::<TypeNode>::relative(array, 0).into();
	assert_eq!(
		render_type_for_compact_tail(&tail, 8, "State.values")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::Vec<u64, 8>"
	);
}

#[test]
fn renders_compact_string_tails_with_default_and_explicit_prefixes() {
	for (format, expected) in [
		(U8, "pina::String<32>"),
		(NumberFormat::U16, "pina::PodString<32, 2>"),
		(NumberFormat::U32, "pina::PodString<32, 4>"),
		(NumberFormat::U64, "pina::PodString<32, 8>"),
	] {
		let string =
			SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), NumberTypeNode::le(format));
		let tail = PostOffsetTypeNode::<TypeNode>::relative(string, 0).into();
		assert_eq!(
			render_type_for_compact_tail(&tail, 32, "State.title")
				.unwrap_or_else(|error| panic!("render failed: {error}")),
			expected
		);
	}
}

#[test]
fn rejects_non_compact_tail_nodes() {
	let number = TypeNode::from(NumberTypeNode::le(NumberFormat::U64));
	let fixed = TypeNode::from(ArrayTypeNode::fixed(
		NumberTypeNode::le(NumberFormat::U64),
		4,
	));

	for node in [number, fixed] {
		let error = render_type_for_compact_tail(&node, 4, "State.values")
			.expect_err("non-prefixed tail must be rejected");
		assert!(error.to_string().contains("support String, Vec"));
	}
}

#[test]
fn renders_every_supported_compact_tail_shape() {
	let string = TypeNode::from(SizePrefixTypeNode::<TypeNode>::new(
		StringTypeNode::utf8(),
		NumberTypeNode::le(U8),
	));
	assert_eq!(
		render_type_for_compact_tail(&string, 8, "State.name")
			.expect("compact string should render"),
		"pina::String<8>",
	);

	let string_item = FixedSizeTypeNode::<TypeNode>::new(
		SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), NumberTypeNode::le(U8)),
		9,
	);
	let strings = TypeNode::from(ArrayTypeNode::prefixed(
		string_item,
		NumberTypeNode::le(NumberFormat::U16),
	));
	assert_eq!(
		render_type_for_compact_tail(&strings, 4, "State.names")
			.expect("compact string vector should render"),
		"pina::Vec<pina::String<8>, 4>",
	);

	let option = TypeNode::from(OptionTypeNode {
		fixed: None,
		item: Box::new(string),
		prefix: NumberTypeNode::le(U8).into(),
	});
	assert_eq!(
		render_type_for_compact_tail(&option, 8, "State.nickname")
			.expect("compact optional string should render"),
		"Option<pina::String<8>>",
	);
}

#[test]
fn rejects_compact_tail_capacity_beyond_prefix_maximum() {
	let tail = TypeNode::from(ArrayTypeNode::prefixed(
		NumberTypeNode::le(NumberFormat::U64),
		NumberTypeNode::le(U8),
	));
	let overflow = render_type_for_compact_tail(&tail, 256, "State.values")
		.expect_err("capacity beyond prefix maximum must be rejected");
	assert!(
		overflow
			.to_string()
			.contains("exceeds the 1-byte prefix maximum")
	);
}

#[test]
fn rejects_non_utf8_compact_strings() {
	let string = StringTypeNode {
		encoding: BytesEncoding::Base58,
		display: None,
	};
	let tail = TypeNode::from(SizePrefixTypeNode::<TypeNode>::new(
		string,
		NumberTypeNode::le(U8),
	));
	let error = render_type_for_compact_tail(&tail, 32, "State.title")
		.expect_err("non-UTF-8 string must be rejected");
	assert!(error.to_string().contains("support String, Vec"));
}

#[test]
fn renders_semantic_pod_collection_types() {
	let string =
		SizePrefixTypeNode::<TypeNode>::new(StringTypeNode::utf8(), NumberTypeNode::le(U8));
	let string = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(string, 33));
	assert_eq!(
		render_type_for_pod(&string, "ProfileState.name")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::String<32>"
	);

	let values = ArrayTypeNode::prefixed(
		NumberTypeNode::le(NumberFormat::U64),
		NumberTypeNode::le(NumberFormat::U16),
	);
	let values = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(values, 66));
	assert_eq!(
		render_type_for_pod(&values, "ProfileState.tags")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::Vec<u64, 8>"
	);

	let color = FixedSizeTypeNode::<TypeNode>::new(DefinedTypeLinkNode::new("color"), 1);
	let colors = ArrayTypeNode::prefixed(color, NumberTypeNode::le(NumberFormat::U16));
	let colors = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(colors, 10));
	assert_eq!(
		render_type_for_pod(&colors, "Palette.colors")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::Vec<crate::generated::types::Color, 8>"
	);
}

#[test]
fn renders_semantic_pod_option_types() {
	let native_option =
		TypeNode::from(OptionTypeNode::fixed(NumberTypeNode::le(NumberFormat::U64)));
	assert_eq!(
		render_type_for_pod(&native_option, "ProfileState.favorite_tag")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"Option<u64>"
	);

	let explicit_option = TypeNode::from(OptionTypeNode {
		fixed: Some(true),
		item: Box::new(NumberTypeNode::le(NumberFormat::U64).into()),
		prefix: NumberTypeNode::le(NumberFormat::U16).into(),
	});
	assert_eq!(
		render_type_for_pod(&explicit_option, "ProfileState.legacy_tag")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::PodOption<<u64 as pina::ZcField>::Pod, 2>"
	);

	let u32_option = TypeNode::from(OptionTypeNode {
		fixed: Some(true),
		item: Box::new(NumberTypeNode::le(U8).into()),
		prefix: NumberTypeNode::le(NumberFormat::U32).into(),
	});
	assert_eq!(
		render_type_for_pod(&u32_option, "ProfileState.compatibility_flag")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::PodOption<<u8 as pina::ZcField>::Pod, 4>"
	);

	let options = ArrayTypeNode::prefixed(
		OptionTypeNode::fixed(NumberTypeNode::le(NumberFormat::U16)),
		NumberTypeNode::le(NumberFormat::U16),
	);
	let options = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(options, 11));
	assert_eq!(
		render_type_for_pod(&options, "ProfileState.values")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::Vec<Option<u16>, 3>"
	);

	let u16_options = ArrayTypeNode::prefixed(
		OptionTypeNode {
			fixed: Some(true),
			item: Box::new(NumberTypeNode::le(U8).into()),
			prefix: NumberTypeNode::le(NumberFormat::U16).into(),
		},
		NumberTypeNode::le(NumberFormat::U16),
	);
	let u16_options = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(u16_options, 8));
	assert_eq!(
		render_type_for_pod(&u16_options, "ProfileState.legacy_flags")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::Vec<pina::PodOption<<u8 as pina::ZcField>::Pod, 2>, 2>"
	);

	let wide_options = ArrayTypeNode::prefixed(
		OptionTypeNode {
			fixed: Some(true),
			item: Box::new(NumberTypeNode::le(U8).into()),
			prefix: NumberTypeNode::le(NumberFormat::U32).into(),
		},
		NumberTypeNode::le(NumberFormat::U16),
	);
	let wide_options = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(wide_options, 12));
	assert_eq!(
		render_type_for_pod(&wide_options, "ProfileState.compatibility_flags")
			.unwrap_or_else(|error| panic!("render failed: {error}")),
		"pina::Vec<pina::PodOption<<u8 as pina::ZcField>::Pod, 4>, 2>"
	);
}

#[test]
fn rejects_non_pinapod_option_layouts() {
	let variable = TypeNode::from(OptionTypeNode::new(NumberTypeNode::le(NumberFormat::U64)));
	let error = render_type_for_pod(&variable, "State.value")
		.expect_err("variable option must not render as PodOption");
	assert!(error.to_string().contains("fixed-size value slot"));

	let variable_item = TypeNode::from(OptionTypeNode::fixed(StringTypeNode::utf8()));
	let error = render_type_for_pod(&variable_item, "State.value")
		.expect_err("variable option value must not render as PodOption");
	assert!(error.to_string().contains("fixed byte size"));

	for prefix in [
		NumberTypeNode::be(NumberFormat::U16),
		NumberTypeNode::le(NumberFormat::U64),
	] {
		let option = TypeNode::from(OptionTypeNode {
			fixed: Some(true),
			item: Box::new(NumberTypeNode::le(NumberFormat::U64).into()),
			prefix: prefix.into(),
		});
		assert!(render_type_for_pod(&option, "State.value").is_err());
	}

	let invalid_nested_option = OptionTypeNode {
		fixed: Some(true),
		item: Box::new(NumberTypeNode::le(NumberFormat::U64).into()),
		prefix: NumberTypeNode::le(NumberFormat::U64).into(),
	};
	let values =
		ArrayTypeNode::prefixed(invalid_nested_option, NumberTypeNode::le(NumberFormat::U16));
	let values = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(values, 18));
	assert!(render_type_for_pod(&values, "State.values").is_err());

	let overflowing_option = OptionTypeNode::fixed(FixedSizeTypeNode::<TypeNode>::new(
		BytesTypeNode::new(),
		usize::MAX,
	));
	let values = ArrayTypeNode::prefixed(overflowing_option, NumberTypeNode::le(NumberFormat::U16));
	let values = TypeNode::from(FixedSizeTypeNode::<TypeNode>::new(values, 2));
	assert!(render_type_for_pod(&values, "State.values").is_err());
}

#[test]
fn renders_instruction_data_with_discriminator_prefix() {
	let crate_dir = render_fixture_program("todo_program", "pina-codama-render-todo");
	let content = read_generated_file(&crate_dir, "instructions/initialize.rs");

	insta::assert_snapshot!("todo_initialize_instruction_rs", content);
}

#[test]
fn renders_root_mod_with_unused_program_reexport_allowance() {
	let crate_dir = render_fixture_program("declare_id_program", "pina-codama-render-root-mod");
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
		events: vec![],
		constants: vec![],
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
fn renders_instruction_account_default_from_pda() {
	let mut state = InstructionAccountNode::new("state", true, false);
	state.default_value = Box::new(Some(InstructionInputValueNode::PdaValue(
		PdaValueNode::new(
			PdaLinkNode::new("statePda"),
			vec![PdaSeedValueNode {
				name: "authority".into(),
				value: Box::new(AccountValueNode::new("authority").into()),
			}],
		),
	)));

	let mut authority = InstructionAccountNode::new("authority", false, true);
	authority.is_signer = IsSigner::Either;

	let program = ProgramNode {
		name: "defaultProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![],
		instructions: vec![InstructionNode {
			name: "initialize".into(),
			docs: Docs::default(),
			optional_account_strategy: Some(OptionalAccountStrategy::ProgramId),
			accounts: vec![authority, state],
			arguments: vec![],
			extra_arguments: vec![],
			remaining_accounts: vec![],
			byte_deltas: vec![],
			discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(7u8)),
				0,
			))],
			status: None,
			sub_instructions: vec![],
			provides: vec![],
			display: None,
			plugins: vec![],
		}],
		defined_types: vec![],
		pdas: vec![PdaNode::new(
			"statePda",
			vec![
				PdaSeedNode::Constant(ConstantPdaSeedNode::new(
					StringTypeNode::utf8(),
					StringValueNode::new("state"),
				)),
				PdaSeedNode::Variable(VariablePdaSeedNode::new(
					"authority",
					PublicKeyTypeNode::new(),
				)),
			],
		)],
		errors: vec![],
		events: vec![],
		constants: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let output_dir = unique_temp_dir("pina-codama-render-pda-default");
	let crate_dir = output_dir.join("default_program");
	render_root_node(
		&RootNode::new(program),
		&crate_dir,
		&RenderConfig::default(),
	)
	.unwrap_or_else(|e| panic!("render failed: {e}"));

	let content = read_generated_file(&crate_dir, "instructions/initialize.rs");
	insta::assert_snapshot!("instruction_account_default_from_pda", content);
}

fn render_optional_accounts(
	optional_account_strategy: Option<OptionalAccountStrategy>,
	prefix: &str,
) -> String {
	let mut optional_signer = InstructionAccountNode::new("optionalSigner", false, false);
	optional_signer.is_optional = Some(true);
	optional_signer.is_signer = IsSigner::Either;
	let required_account = InstructionAccountNode::new("requiredAccount", false, false);

	let program = ProgramNode {
		name: "optionalProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![],
		instructions: vec![InstructionNode {
			name: "maybe".into(),
			docs: Docs::default(),
			optional_account_strategy,
			accounts: vec![optional_signer, required_account],
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
			provides: vec![],
			display: None,
			plugins: vec![],
		}],
		defined_types: vec![],
		pdas: vec![],
		errors: vec![],
		events: vec![],
		constants: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let output_dir = unique_temp_dir(prefix);
	let crate_dir = output_dir.join("optional_program");
	render_root_node(
		&RootNode::new(program),
		&crate_dir,
		&RenderConfig::default(),
	)
	.unwrap_or_else(|e| panic!("render failed: {e}"));

	read_generated_file(&crate_dir, "instructions/maybe.rs")
}

fn assert_optional_account_meta_order(content: &str, expects_program_fallback: bool) {
	let optional_meta = "new_readonly(optional_signer, signer)";
	let fallback_meta = "new_readonly(crate::OPTIONAL_PROGRAM_ID, false)";
	let required_meta = "new_readonly(self.required_account, false)";
	let optional_index = content
		.find(optional_meta)
		.unwrap_or_else(|| panic!("missing optional account meta:\n{content}"));
	let required_index = content
		.find(required_meta)
		.unwrap_or_else(|| panic!("missing required account meta:\n{content}"));

	assert!(
		optional_index < required_index,
		"optional account meta must precede the required account meta"
	);

	if expects_program_fallback {
		let fallback_index = content
			.find(fallback_meta)
			.unwrap_or_else(|| panic!("missing program fallback meta:\n{content}"));

		assert!(
			optional_index < fallback_index && fallback_index < required_index,
			"program fallback meta must preserve the optional account position"
		);
	} else {
		assert!(
			!content.contains(fallback_meta),
			"omitted strategy must not render a program fallback meta"
		);
	}
}

#[test]
fn renders_optional_accounts_with_program_fallback_strategy() {
	let content = render_optional_accounts(
		Some(OptionalAccountStrategy::ProgramId),
		"pina-codama-render-optional-fallback",
	);
	assert_optional_account_meta_order(&content, true);

	insta::assert_snapshot!("optional_accounts_with_program_fallback_strategy", content);
}

#[test]
fn defaults_optional_accounts_to_program_fallback_strategy() {
	let default_content = render_optional_accounts(None, "pina-codama-render-optional-default");
	let explicit_content = render_optional_accounts(
		Some(OptionalAccountStrategy::ProgramId),
		"pina-codama-render-optional-explicit",
	);

	assert_eq!(default_content, explicit_content);
	assert_optional_account_meta_order(&default_content, true);
}

#[test]
fn renders_optional_accounts_with_omitted_strategy() {
	let content = render_optional_accounts(
		Some(OptionalAccountStrategy::Omitted),
		"pina-codama-render-optional-omitted",
	);

	assert_optional_account_meta_order(&content, false);
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
		events: vec![],
		constants: vec![],
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
					endian: Endianness::Be,
					display: Box::new(None),
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
		events: vec![],
		constants: vec![],
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
			r#type: Box::new(TypeNode::Number(NumberTypeNode {
				format: NumberFormat::U64,
				endian: Endianness::Le,
				display: Box::new(None),
			})),
		}],
		pdas: vec![],
		errors: vec![],
		events: vec![],
		constants: vec![],
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
fn renders_defined_structs_with_the_pinapod_crate_path() {
	let defined = DefinedTypeNode {
		name: "settings".into(),
		docs: vec!["Shared settings.".to_string()].into(),
		r#type: Box::new(
			StructTypeNode::new(vec![StructFieldTypeNode::new(
				"counter",
				NumberTypeNode::le(NumberFormat::U64),
			)])
			.into(),
		),
	};

	let content = render_defined_type_page(&defined)
		.unwrap_or_else(|error| panic!("defined struct render failed: {error}"));
	assert!(content.contains("#[derive(pina::PinaPod)]"));
	assert!(content.contains("#[pinapod(crate = pina::pinapod, no_inherent)]"));
	assert!(content.contains("pub counter: u64,"));
}

#[test]
fn renders_pinapod_enum_defined_type() {
	let mut red = EnumEmptyVariantTypeNode::new("red");
	red.discriminator = Some(0);
	let mut blue = EnumEmptyVariantTypeNode::new("blue");
	blue.discriminator = Some(1);
	let color = TypeNode::Link(DefinedTypeLinkNode::new("color"));
	let color_item = FixedSizeTypeNode::<TypeNode>::new(color.clone(), 1);
	let colors = ArrayTypeNode::prefixed(color_item, NumberTypeNode::le(NumberFormat::U16));
	let colors = FixedSizeTypeNode::<TypeNode>::new(colors, 10);
	let program = ProgramNode {
		name: "podEnumProgram".into(),
		public_key: "11111111111111111111111111111111".to_string(),
		accounts: vec![AccountNode {
			name: "palette".into(),
			size: None,
			docs: Docs::default(),
			data: StructTypeNode::new(vec![
				StructFieldTypeNode::new("discriminator", NumberTypeNode::le(U8)),
				StructFieldTypeNode::new("color", color),
				StructFieldTypeNode::new("colors", colors),
			])
			.into(),
			pda: None,
			discriminators: vec![DiscriminatorNode::Constant(ConstantDiscriminatorNode::new(
				ConstantValueNode::new(NumberTypeNode::le(U8), NumberValueNode::new(1u8)),
				0,
			))],
		}],
		instructions: vec![],
		defined_types: vec![DefinedTypeNode {
			name: "color".into(),
			docs: vec!["A color stored on chain.".to_string()].into(),
			r#type: Box::new(
				EnumTypeNode {
					variants: vec![
						EnumVariantTypeNode::Empty(red),
						EnumVariantTypeNode::Empty(blue),
					],
					size: NumberTypeNode::le(U8).into(),
				}
				.into(),
			),
		}],
		pdas: vec![],
		errors: vec![],
		events: vec![],
		constants: vec![],
		version: String::new(),
		origin: None,
		docs: Docs::default(),
	};
	let root = RootNode::new(program);
	let output_dir = unique_temp_dir("pina-codama-render-pod-enum");
	let crate_dir = output_dir.join("pinapod_enum_program");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("render failed: {error}"));

	let content = read_generated_file(&crate_dir, "types/color.rs");
	assert!(content.contains("#[derive(Clone, Copy, Debug, PartialEq, Eq, pina::PinaPod)]"));
	assert!(content.contains("pub enum Color"));
	assert!(content.contains("Red = 0"));
	assert!(content.contains("Blue = 1"));
	let account = read_generated_file(&crate_dir, "accounts/palette.rs");
	assert!(account.contains("pub color: crate::generated::types::Color"));
	assert!(account.contains("pub colors: pina::Vec<crate::generated::types::Color, 8>"));
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
			optional_account_strategy: Some(OptionalAccountStrategy::ProgramId),
			accounts: vec![InstructionAccountNode::new("payer", true, true)],
			arguments: vec![],
			extra_arguments: vec![],
			remaining_accounts: vec![],
			byte_deltas: vec![],
			discriminators: vec![],
			status: None,
			sub_instructions: vec![],
			provides: vec![],
			display: None,
			plugins: vec![],
		}],
		defined_types: vec![],
		pdas: vec![],
		errors: vec![],
		events: vec![],
		constants: vec![],
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

	insta::assert_snapshot!(
		"rejects_missing_instruction_discriminators",
		err.to_string()
	);
}

#[test]
fn writes_scaffold_with_pinapod_dependency() {
	let root = load_fixture_root("hello_solana_program");
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

	insta::assert_snapshot!("writes_scaffold_with_pinapod_dependency", cargo_toml);
}

#[test]
fn generation_modes_preserve_or_replace_scaffolds() {
	let root = load_fixture_root("hello_solana_program");
	let crate_dir = unique_temp_dir("pina-codama-render-modes");
	let create = RenderConfig {
		mode: RenderMode::Create,
		..RenderConfig::default()
	};
	render_root_node(&root, &crate_dir, &create)
		.unwrap_or_else(|error| panic!("create failed: {error}"));
	fs::write(crate_dir.join("Cargo.toml"), "# consumer manifest\n")
		.unwrap_or_else(|error| panic!("manifest edit failed: {error}"));
	fs::write(crate_dir.join("src/lib.rs"), "// consumer entrypoint\n")
		.unwrap_or_else(|error| panic!("entrypoint edit failed: {error}"));

	let create_error = render_root_node(&root, &crate_dir, &create)
		.expect_err("create must reject a nonempty destination");
	assert!(matches!(
		create_error,
		RenderError::InvalidGenerationState { .. }
	));

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
	assert_eq!(
		fs::read_to_string(crate_dir.join("src/lib.rs"))
			.unwrap_or_else(|error| panic!("entrypoint read failed: {error}")),
		"// consumer entrypoint\n"
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
fn source_only_generation_and_strict_update_are_explicit() {
	let root = load_fixture_root("hello_solana_program");
	let crate_dir = unique_temp_dir("pina-codama-render-source-only");
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
fn generation_mode_labels_and_error_helpers_cover_the_public_policy() {
	assert_eq!(RenderMode::Auto.as_str(), "automatically generate");
	assert_eq!(RenderMode::Create.as_str(), "create");
	assert_eq!(RenderMode::Update.as_str(), "update");
	assert_eq!(RenderMode::Overwrite.as_str(), "overwrite");
	assert!(matches!(
		read_file_error(Path::new("input"), std::io::Error::other("read")),
		RenderError::ReadFile { .. }
	));
	assert!(matches!(
		write_file_error(Path::new("output"), std::io::Error::other("write")),
		RenderError::WriteFile { .. }
	));
}

#[cfg(unix)]
#[test]
fn generation_modes_reject_unsafe_destination_trees_and_unreadable_paths() {
	use std::os::unix::fs::PermissionsExt;
	use std::os::unix::fs::symlink;

	let root = load_fixture_root("hello_solana_program");
	let output = unique_temp_dir("pina-codama-render-mode-safety");
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
fn validates_boolean_encoding_for_pda_variable_seed() {
	let boolean_type = BooleanTypeNode {
		size: NumberTypeNode {
			format: NumberFormat::U16,
			endian: Endianness::Le,
			display: Box::new(None),
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

	insta::assert_snapshot!(
		"validates_boolean_encoding_for_pda_variable_seed",
		err.to_string()
	);
}

#[test]
fn snapshots_escrow_take_instruction() {
	let crate_dir = render_fixture_program("escrow_program", "pina-codama-render-escrow");
	let content = read_generated_file(&crate_dir, "instructions/take.rs");
	insta::assert_snapshot!("escrow_take_instruction_rs", content);
}

#[test]
fn renders_untrusted_multiline_text_as_valid_rust() {
	let mut account_root = load_fixture_root("counter_program");
	account_root.program.accounts[0]
		.docs
		.push("Account documentation.\npub const INJECTED: bool = true;");
	let account_files = render_program_to_files(&account_root)
		.unwrap_or_else(|error| panic!("render failed: {error}"));
	let account_source = account_files
		.get(Path::new("accounts/counter_state.rs"))
		.unwrap_or_else(|| panic!("missing account source"));
	assert!(account_source.contains("/// pub const INJECTED: bool = true;"));
	assert!(!account_source.contains("\npub const INJECTED: bool = true;"));
	syn::parse_file(account_source)
		.unwrap_or_else(|error| panic!("generated account source is invalid: {error}"));

	let mut error_root = load_fixture_root("custom_errors_program");
	error_root.program.errors[0].message =
		"bad message\n)]\npub const INJECTED: bool = true;".to_string();
	let error_files = render_program_to_files(&error_root)
		.unwrap_or_else(|error| panic!("render failed: {error}"));
	let error_path = format!("errors/{}.rs", snake(error_root.program.name.as_ref()));
	let error_source = error_files
		.get(Path::new(&error_path))
		.unwrap_or_else(|| panic!("missing error source"));
	assert!(error_source.contains("/// pub const INJECTED: bool = true;"));
	assert!(!error_source.contains("\npub const INJECTED: bool = true;"));
	syn::parse_file(error_source)
		.unwrap_or_else(|error| panic!("generated error source is invalid: {error}"));
}

#[test]
fn rejects_output_path_escape_without_mutating_files() {
	let root = load_fixture_root("counter_program");
	let output_dir = unique_temp_dir("pina-codama-render-path-escape");
	let crate_dir = output_dir.join("client");
	let escaped_dir = output_dir.join("escaped");
	fs::create_dir_all(&escaped_dir)
		.unwrap_or_else(|error| panic!("failed to create escaped directory: {error}"));
	let sentinel_path = escaped_dir.join("sentinel.txt");
	fs::write(&sentinel_path, "keep")
		.unwrap_or_else(|error| panic!("failed to write sentinel: {error}"));

	let error = render_root_node(
		&root,
		&crate_dir,
		&RenderConfig {
			delete_folder_before_rendering: true,
			generated_folder: PathBuf::from("../escaped"),
			..RenderConfig::default()
		},
	)
	.err()
	.unwrap_or_else(|| panic!("expected unsafe output path to be rejected"));
	assert!(matches!(error, RenderError::UnsafeOutputPath { .. }));
	assert_eq!(
		fs::read_to_string(&sentinel_path)
			.unwrap_or_else(|error| panic!("failed to read sentinel: {error}")),
		"keep"
	);
	assert!(!crate_dir.exists());

	let absolute_error = render_root_node(
		&root,
		&crate_dir,
		&RenderConfig {
			delete_folder_before_rendering: true,
			generated_folder: escaped_dir.clone(),
			..RenderConfig::default()
		},
	)
	.err()
	.unwrap_or_else(|| panic!("expected absolute output path to be rejected"));
	assert!(matches!(
		absolute_error,
		RenderError::UnsafeOutputPath { .. }
	));
	assert_eq!(
		fs::read_to_string(&sentinel_path)
			.unwrap_or_else(|error| panic!("failed to reread sentinel: {error}")),
		"keep"
	);
}

#[test]
fn refuses_to_delete_an_unmanaged_directory() {
	let root = load_fixture_root("counter_program");
	let crate_dir = unique_temp_dir("pina-codama-render-unmanaged");
	let source_dir = crate_dir.join("src");
	fs::create_dir_all(&source_dir)
		.unwrap_or_else(|error| panic!("failed to create source directory: {error}"));
	let sentinel_path = source_dir.join("lib.rs");
	fs::write(&sentinel_path, "pub const KEEP: bool = true;")
		.unwrap_or_else(|error| panic!("failed to write source sentinel: {error}"));

	let error = render_root_node(
		&root,
		&crate_dir,
		&RenderConfig {
			delete_folder_before_rendering: true,
			generated_folder: PathBuf::from("src"),
			..RenderConfig::default()
		},
	)
	.err()
	.unwrap_or_else(|| panic!("expected unmanaged source directory to be rejected"));
	assert!(matches!(error, RenderError::UnsafeOutputPath { .. }));
	assert_eq!(
		fs::read_to_string(&sentinel_path)
			.unwrap_or_else(|error| panic!("failed to read source sentinel: {error}")),
		"pub const KEEP: bool = true;"
	);
}

#[test]
fn refuses_to_delete_unmanaged_files_inside_a_generated_directory() {
	let root = load_fixture_root("counter_program");
	let crate_dir = unique_temp_dir("pina-codama-render-mixed-output");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("initial render failed: {error}"));
	let sentinel_path = crate_dir.join("src/generated/keep.txt");
	fs::write(&sentinel_path, "user-authored")
		.unwrap_or_else(|error| panic!("failed to write user file: {error}"));

	let error = render_root_node(&root, &crate_dir, &RenderConfig::default())
		.err()
		.unwrap_or_else(|| panic!("expected mixed generated directory to be rejected"));

	assert!(matches!(error, RenderError::UnsafeOutputPath { .. }));
	assert_eq!(
		fs::read_to_string(&sentinel_path)
			.unwrap_or_else(|error| panic!("failed to read user file: {error}")),
		"user-authored"
	);
}

#[test]
fn render_failure_preserves_last_known_good_output() {
	let root = load_fixture_root("counter_program");
	let crate_dir = unique_temp_dir("pina-codama-render-preserve");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("initial render failed: {error}"));
	let generated_path = crate_dir.join("src/generated/programs.rs");
	let previous_source = fs::read_to_string(&generated_path)
		.unwrap_or_else(|error| panic!("failed to read generated source: {error}"));
	let cargo_path = crate_dir.join("Cargo.toml");
	fs::write(&cargo_path, "last-known-good")
		.unwrap_or_else(|error| panic!("failed to write Cargo sentinel: {error}"));

	let mut invalid_root = root;
	invalid_root.program.public_key =
		"11111111111111111111111111111111\"); pub const INJECTED: bool = true; //".to_string();
	let error = render_root_node(&invalid_root, &crate_dir, &RenderConfig::default())
		.err()
		.unwrap_or_else(|| panic!("expected invalid public key to fail"));
	assert!(matches!(error, RenderError::UnsupportedValue { .. }));
	assert_eq!(
		fs::read_to_string(&generated_path)
			.unwrap_or_else(|error| panic!("failed to reread generated source: {error}")),
		previous_source
	);
	assert_eq!(
		fs::read_to_string(&cargo_path)
			.unwrap_or_else(|error| panic!("failed to reread Cargo sentinel: {error}")),
		"last-known-good"
	);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_generated_directory() {
	use std::os::unix::fs::symlink;

	let root = load_fixture_root("counter_program");
	let output_dir = unique_temp_dir("pina-codama-render-symlink");
	let crate_dir = output_dir.join("client");
	let external_dir = output_dir.join("external");
	fs::create_dir_all(crate_dir.join("src"))
		.unwrap_or_else(|error| panic!("failed to create crate directory: {error}"));
	fs::create_dir_all(&external_dir)
		.unwrap_or_else(|error| panic!("failed to create external directory: {error}"));
	let sentinel_path = external_dir.join("sentinel.txt");
	fs::write(&sentinel_path, "keep")
		.unwrap_or_else(|error| panic!("failed to write sentinel: {error}"));
	symlink(&external_dir, crate_dir.join("src/generated"))
		.unwrap_or_else(|error| panic!("failed to create symlink: {error}"));

	let error = render_root_node(&root, &crate_dir, &RenderConfig::default())
		.err()
		.unwrap_or_else(|| panic!("expected symlinked output path to fail"));
	assert!(matches!(error, RenderError::UnsafeOutputPath { .. }));
	assert_eq!(
		fs::read_to_string(&sentinel_path)
			.unwrap_or_else(|error| panic!("failed to read sentinel: {error}")),
		"keep"
	);
}

#[cfg(unix)]
#[test]
fn rejects_dangling_symlinked_generated_directory() {
	use std::os::unix::fs::symlink;

	let root = load_fixture_root("counter_program");
	let output_dir = unique_temp_dir("pina-codama-render-dangling-symlink");
	let crate_dir = output_dir.join("client");
	let dangling_target = output_dir.join("external/does-not-exist");
	fs::create_dir_all(crate_dir.join("src"))
		.unwrap_or_else(|error| panic!("failed to create crate directory: {error}"));
	symlink(&dangling_target, crate_dir.join("src/generated"))
		.unwrap_or_else(|error| panic!("failed to create dangling symlink: {error}"));

	let error = render_root_node(&root, &crate_dir, &RenderConfig::default())
		.err()
		.unwrap_or_else(|| panic!("expected dangling symlinked output path to fail"));

	assert!(matches!(error, RenderError::UnsafeOutputPath { .. }));
	assert!(!dangling_target.exists());
}
