mod error;
mod render;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use codama_nodes::ProgramNode;
use codama_nodes::RootNode;
pub use error::RenderError;
pub use error::Result;
use render::*;

#[derive(Clone, Debug)]
pub struct RenderConfig {
	pub delete_folder_before_rendering: bool,
	pub generated_folder: PathBuf,
}

impl Default for RenderConfig {
	fn default() -> Self {
		Self {
			delete_folder_before_rendering: true,
			generated_folder: PathBuf::from("src/generated"),
		}
	}
}

pub fn read_root_node(path: &Path) -> Result<RootNode> {
	let idl = fs::read_to_string(path).map_err(|source| {
		RenderError::ReadFile {
			path: path.to_path_buf(),
			source,
		}
	})?;
	serde_json::from_str(&idl).map_err(|source| {
		RenderError::ParseIdl {
			path: path.to_path_buf(),
			source,
		}
	})
}

pub fn render_idl_file(path: &Path, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	let root = read_root_node(path)?;
	render_root_node(&root, crate_dir, config)
}

pub fn render_root_node(root: &RootNode, crate_dir: &Path, config: &RenderConfig) -> Result<()> {
	ensure_crate_scaffold(crate_dir, root.program.name.as_ref())?;
	let generated_dir = crate_dir.join(&config.generated_folder);

	if config.delete_folder_before_rendering && generated_dir.exists() {
		fs::remove_dir_all(&generated_dir).map_err(|source| {
			RenderError::WriteFile {
				path: generated_dir.clone(),
				source,
			}
		})?;
	}

	let files = render_program_to_files(root)?;
	write_files(&generated_dir, files)
}

pub fn render_program(
	program: &ProgramNode,
	crate_dir: &Path,
	config: &RenderConfig,
) -> Result<()> {
	let root = RootNode::new(program.clone());
	render_root_node(&root, crate_dir, config)
}

fn render_program_to_files(root: &RootNode) -> Result<BTreeMap<PathBuf, String>> {
	let program = &root.program;
	let mut files = BTreeMap::new();

	let program_constants = std::iter::once(&root.program)
		.chain(root.additional_programs.iter())
		.collect::<Vec<_>>();
	let primary_program_const = program_id_const_name(program.name.as_ref());

	let pdas_by_name = program
		.pdas
		.iter()
		.map(|pda| (pda.name.as_ref().to_string(), pda))
		.collect::<BTreeMap<_, _>>();

	files.insert(PathBuf::from("mod.rs"), page(&render_root_mod(program)));
	files.insert(
		PathBuf::from("programs.rs"),
		page(&render_programs_mod(&program_constants)),
	);

	if !program.accounts.is_empty() {
		files.insert(
			PathBuf::from("accounts/mod.rs"),
			page(&render_accounts_mod(&program.accounts)),
		);

		for account in &program.accounts {
			let filename = format!("accounts/{}.rs", snake(account.name.as_ref()));
			let account_content = render_account_page(
				account,
				&primary_program_const,
				pdas_by_name
					.get(account.pda.as_ref().map_or("", |p| p.name.as_ref()))
					.copied(),
			)?;
			files.insert(PathBuf::from(filename), page(&account_content));
		}
	}

	if !program.instructions.is_empty() {
		files.insert(
			PathBuf::from("instructions/mod.rs"),
			page(&render_instructions_mod(&program.instructions)),
		);
		for instruction in &program.instructions {
			let filename = format!("instructions/{}.rs", snake(instruction.name.as_ref()));
			let instruction_content =
				render_instruction_page(instruction, program, &primary_program_const)?;
			files.insert(PathBuf::from(filename), page(&instruction_content));
		}
	}

	if !program.defined_types.is_empty() {
		files.insert(
			PathBuf::from("types/mod.rs"),
			page(&render_defined_types_mod(&program.defined_types)),
		);
		for defined_type in &program.defined_types {
			let filename = format!("types/{}.rs", snake(defined_type.name.as_ref()));
			let defined_type_content = render_defined_type_page(defined_type)?;
			files.insert(PathBuf::from(filename), page(&defined_type_content));
		}
	}

	if !program.errors.is_empty() {
		files.insert(
			PathBuf::from("errors/mod.rs"),
			page(&render_errors_mod(program)),
		);
		files.insert(
			PathBuf::from(format!("errors/{}.rs", snake(program.name.as_ref()))),
			page(&render_errors_page(program)),
		);
	}

	Ok(files)
}

#[cfg(test)]
mod tests {
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

		assert!(content.contains("use bytemuck::Pod;"));
		assert!(content.contains("pub discriminator: u8,"));
		assert!(content.contains("pub count: pina_pod_primitives::PodU64,"));
		assert!(content.contains("pub const COUNTER_STATE_DISCRIMINATOR: u8 = 1u8;"));
		assert!(content.contains("bytemuck::try_from_bytes::<Self>(data)"));
		insta::assert_snapshot!("counter_state_account_rs", content);
	}

	#[test]
	fn renders_instruction_data_with_discriminator_prefix() {
		let crate_dir = render_fixture_program("todo_program", "pina-codama-render-todo");
		let content = read_generated_file(&crate_dir, "instructions/initialize.rs");

		assert!(content.contains("pub const INITIALIZE_DISCRIMINATOR: u8 = 0u8;"));
		assert!(content.contains("pub struct InitializeInstructionData {"));
		assert!(content.contains("pub discriminator: u8,"));
		assert!(content.contains("pub digest: [u8; 32],"));
		assert!(content.contains("let data = bytemuck::bytes_of(&data).to_vec();"));
		insta::assert_snapshot!("todo_initialize_instruction_rs", content);
	}

	#[test]
	fn renders_root_mod_with_unused_program_reexport_allowance() {
		let crate_dir = render_fixture_program("anchor_declare_id", "pina-codama-render-root-mod");
		let content = read_generated_file(&crate_dir, "mod.rs");

		assert!(content.contains("#[allow(unused_imports)]"));
		assert!(content.contains("pub(crate) use programs::*;"));
	}

	#[test]
	fn renders_instruction_account_metas_using_self_fields() {
		let crate_dir =
			render_fixture_program("counter_program", "pina-codama-render-self-account-metas");
		let initialize_content = read_generated_file(&crate_dir, "instructions/initialize.rs");
		let increment_content = read_generated_file(&crate_dir, "instructions/increment.rs");

		assert!(initialize_content.contains("AccountMeta::new_readonly(self.authority, true)"));
		assert!(initialize_content.contains("AccountMeta::new(self.counter, false)"));
		assert!(
			initialize_content.contains("AccountMeta::new_readonly(self.system_program, false)")
		);

		assert!(increment_content.contains("AccountMeta::new_readonly(self.authority, true)"));
		assert!(increment_content.contains("AccountMeta::new(self.counter, false)"));
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

		assert!(content.contains("pub fn find_pda("));
		assert!(content.contains("pub fn create_pda("));
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
		render_root_node(
			&RootNode::new(program),
			&crate_dir,
			&RenderConfig::default(),
		)
		.unwrap_or_else(|e| panic!("render failed: {e}"));

		let content = read_generated_file(&crate_dir, "instructions/maybe.rs");
		assert!(
			content.contains("if let Some((optional_signer, signer)) = self.optional_signer {")
		);
		assert!(content.contains("crate::OPTIONAL_PROGRAM_ID"));
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
		assert!(
			err.to_string()
				.contains("variable-size strings are not POD"),
			"unexpected error: {err}"
		);
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

		assert!(
			err.to_string()
				.contains("only little-endian number types are supported"),
			"unexpected error: {err}"
		);
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
		assert!(content.contains("pub type Counter = pina_pod_primitives::PodU64;"));
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

		assert!(
			err.to_string().contains("missing required discriminator"),
			"unexpected error: {err}"
		);
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
		assert!(cargo_toml.contains("bytemuck = { workspace = true , default-features = true }"));
		assert!(cargo_toml.contains("solana-cpi = { workspace = true , default-features = true }"));
		assert!(cargo_toml.contains("pina_pod_primitives = { workspace = true }"));
		assert!(!cargo_toml.contains("borsh = { workspace = true }"));
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

		assert!(
			err.to_string()
				.contains("booleans must use little-endian u8 for PDA seeds"),
			"unexpected error: {err}"
		);
	}

	#[test]
	fn snapshots_escrow_take_instruction() {
		let crate_dir = render_fixture_program("escrow_program", "pina-codama-render-escrow");
		let content = read_generated_file(&crate_dir, "instructions/take.rs");
		insta::assert_snapshot!("escrow_take_instruction_rs", content);
	}
}
