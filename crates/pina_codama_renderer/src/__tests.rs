use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use codama_nodes::AccountNode;
use codama_nodes::AccountValueNode;
use codama_nodes::ArrayTypeNode;
use codama_nodes::BooleanTypeNode;
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

use super::render::seeds::render_variable_seed_parameter;
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

#[test]
fn renders_counter_account_with_pod_types() {
	let crate_dir = render_fixture_program("counter_program", "pina-codama-render-counter");
	let content = read_generated_file(&crate_dir, "accounts/counter_state.rs");

	insta::assert_snapshot!("counter_state_account_rs", content);
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
fn rejects_non_zeropod_option_layouts() {
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
		codama_nodes::BytesTypeNode::new(),
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
fn renders_zeropod_enum_defined_type() {
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
	let crate_dir = output_dir.join("zeropod_enum_program");
	render_root_node(&root, &crate_dir, &RenderConfig::default())
		.unwrap_or_else(|error| panic!("render failed: {error}"));

	let content = read_generated_file(&crate_dir, "types/color.rs");
	assert!(content.contains("#[derive(Clone, Copy, Debug, PartialEq, Eq, pina::ZeroPod)]"));
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
fn writes_scaffold_with_zeropod_dependency() {
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

	insta::assert_snapshot!("writes_scaffold_with_zeropod_dependency", cargo_toml);
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

	let mut error_root = load_fixture_root("anchor_errors");
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
