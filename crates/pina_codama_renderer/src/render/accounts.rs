use std::fmt::Write as _;

use codama_nodes::AccountNode;
use codama_nodes::DefaultValueStrategy;
use codama_nodes::NestedTypeNodeTrait;
use codama_nodes::Number;
use codama_nodes::NumberFormat;
use codama_nodes::PdaNode;
use codama_nodes::PdaSeedNode;
use codama_nodes::TypeNode;
use codama_nodes::ValueNode;
use heck::ToShoutySnakeCase as _;

use super::capacity::CompactCapacityIndex;
use super::discriminator::render_constant_discriminator;
use super::discriminator::render_omitted_value_constant;
use super::helpers::pascal;
use super::helpers::render_docs;
use super::helpers::snake;
use super::seeds::render_constant_seed_expression;
use super::seeds::render_variable_seed_parameter;
use super::types::is_compact_tail;
use super::types::render_type_for_compact_tail;
use super::types::render_type_for_pod;
use crate::error::Result;

pub(crate) fn is_compact_account(account: &AccountNode) -> bool {
	account
		.data
		.get_nested_type_node()
		.fields
		.iter()
		.any(|field| is_compact_tail(&field.r#type))
}

pub(crate) fn render_accounts_mod(accounts: &[AccountNode]) -> String {
	let mut lines = Vec::new();

	for account in accounts {
		lines.push(format!(
			"pub(crate) mod r#{};",
			snake(account.name.as_ref())
		));
	}

	lines.push(String::new());

	for account in accounts {
		lines.push(format!(
			"pub use self::r#{}::*;",
			snake(account.name.as_ref())
		));
	}

	lines.join("\n")
}

pub(crate) fn render_account_page(
	account: &AccountNode,
	primary_program_const: &str,
	pda: Option<&PdaNode>,
	compact_capacities: &CompactCapacityIndex,
) -> Result<String> {
	let account_name = pascal(account.name.as_ref());
	let zc_name = format!("{account_name}Zc");
	let context = format!("account `{account_name}`");
	let discriminator =
		render_constant_discriminator(account.name.as_ref(), &account.discriminators, &context)?;

	let data_type = account.data.get_nested_type_node();
	let omitted_constants = data_type
		.fields
		.iter()
		.filter(|field| {
			field.name.as_ref() != "discriminator"
				&& matches!(
					field.default_value_strategy,
					Some(DefaultValueStrategy::Omitted)
				)
		})
		.map(|field| {
			render_omitted_value_constant(
				account.name.as_ref(),
				field.name.as_ref(),
				&field.r#type,
				field.default_value.as_ref().as_ref(),
				&context,
			)
		})
		.collect::<Result<Vec<_>>>()?;
	let first_compact_tail = data_type
		.fields
		.iter()
		.position(|field| is_compact_tail(&field.r#type));
	let compact_account = is_compact_account(account);
	let mut field_lines = Vec::new();
	for doc_line in render_docs(&account.docs, 0) {
		field_lines.push(doc_line);
	}
	if let Some(discriminator) = &discriminator {
		field_lines.push(format!("\tpub discriminator: {},", discriminator.ty));
	}
	for (index, field) in data_type.fields.iter().enumerate() {
		if discriminator.is_some() && field.name.as_ref() == "discriminator" {
			continue;
		}
		let field_name = snake(field.name.as_ref());
		let field_context = format!("{account_name}.{field_name}");
		let field_type = if first_compact_tail.is_some_and(|start| index >= start) {
			let capacity =
				compact_capacities.capacity(account.name.as_ref(), field.name.as_ref())?;
			render_type_for_compact_tail(&field.r#type, capacity, &field_context)?
		} else {
			render_type_for_pod(&field.r#type, &field_context)?
		};
		for doc_line in render_docs(&field.docs, 1) {
			field_lines.push(doc_line);
		}
		field_lines.push(format!("\tpub {field_name}: {field_type},"));
	}

	let mut lines = Vec::new();
	lines.push("#[allow(clippy::len_without_is_empty)]".to_string());
	lines.push("#[derive(pina::PinaPod)]".to_string());
	lines.push("#[pinapod(crate = pina::pinapod, no_inherent)]".to_string());
	if compact_account {
		lines.push("#[pinapod(compact)]".to_string());
	}
	lines.push(format!("pub struct {account_name} {{"));
	lines.extend(field_lines);
	lines.push("}".to_string());
	lines.push(String::new());

	if let Some(discriminator) = &discriminator {
		lines.push(format!(
			"pub const {}: {} = {};",
			discriminator.name, discriminator.ty, discriminator.value
		));
		lines.push(String::new());
	}
	for constant in &omitted_constants {
		lines.push(format!(
			"pub const {}: {} = {};",
			constant.name, constant.ty, constant.value
		));
	}
	if !omitted_constants.is_empty() {
		lines.push(String::new());
	}

	lines.push(format!("impl {account_name} {{"));
	if compact_account {
		lines.extend(render_compact_account_helpers(
			&account_name,
			discriminator.as_ref(),
			&omitted_constants,
		));
	} else {
		lines.extend(render_fixed_account_helpers(
			&zc_name,
			discriminator.as_ref(),
			&omitted_constants,
		));
	}
	lines.push("}".to_string());

	if let Some(pda) = pda {
		lines.push(String::new());
		let helpers =
			render_account_pda_helpers(account_name.as_str(), pda, primary_program_const)?;
		lines.extend(helpers);
	}

	// Migratable accounts also carry a cheap stale-bytes check next to the
	// version constant, so released clients can decide when to send the
	// reserved `Migrate` instruction. Fixed layouts additionally expose a
	// direction-aware decode with generated contract tests.
	if let Some(envelope) = migration_envelope(account) {
		lines.push(String::new());
		lines.push(render_needs_migration(&envelope));
		if !compact_account {
			lines.push(render_try_from_bytes(&envelope));
		}
	}

	Ok(lines.join("\n"))
}

fn render_fixed_account_helpers(
	zc_name: &str,
	discriminator: Option<&super::discriminator::DiscriminatorInfo>,
	omitted_constants: &[super::discriminator::OmittedConstantInfo],
) -> Vec<String> {
	let mut lines = Vec::new();
	lines.push(format!(
		"\tpub const LEN: usize = core::mem::size_of::<{zc_name}>();"
	));
	lines.push(String::new());
	lines.push("\t/// Initialize and validate account storage in one pass.".to_string());
	lines.push("\t///".to_string());
	lines.push(
		"\t/// The destination is cleared again if configuration or validation fails.".to_string(),
	);
	lines.push(format!(
		"\tpub fn initialize(\n\t\tdata: &mut [u8],\n\t\tconfigure: impl FnOnce(&mut \
		 {zc_name}),\n\t) -> Result<&mut {zc_name}, solana_program_error::ProgramError> {{"
	));
	lines.push("\t\t<Self as pina::PinaPodFixed>::initialize(data, |account| {".to_string());
	lines.push("\t\t\tconfigure(account);".to_string());
	if let Some(discriminator) = discriminator {
		lines.push(format!(
			"\t\t\taccount.discriminator = {};",
			discriminator.name
		));
	}
	for constant in omitted_constants {
		lines.push(format!(
			"\t\t\taccount.{} = {};",
			constant.field, constant.name
		));
	}
	lines.push("\t\t\tOk(())".to_string());
	lines.push("\t\t})".to_string());
	lines.push(
		"\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)".to_string(),
	);
	lines.push("\t}".to_string());
	lines.push(String::new());

	lines.push(format!(
		"\tpub fn from_bytes(data: &[u8]) -> Result<&{zc_name}, \
		 solana_program_error::ProgramError> {{"
	));
	lines.push("\t\tlet account = <Self as pina::PinaPodFixed>::read_exact(data)".to_string());
	lines.push(
		"\t\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)?;".to_string(),
	);
	if let Some(discriminator) = discriminator {
		lines.push(format!(
			"\t\tif account.discriminator != {} {{",
			discriminator.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	for constant in omitted_constants {
		lines.push(format!(
			"\t\tif account.{} != {} {{",
			constant.field, constant.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	lines.push("\t\tOk(account)".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());

	lines.push(format!(
		"\tpub fn from_bytes_mut(data: &mut [u8]) -> Result<&mut {zc_name}, \
		 solana_program_error::ProgramError> {{"
	));
	lines.push("\t\tlet account = <Self as pina::PinaPodFixed>::read_exact_mut(data)".to_string());
	lines.push(
		"\t\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)?;".to_string(),
	);
	if let Some(discriminator) = discriminator {
		lines.push(format!(
			"\t\tif account.discriminator != {} {{",
			discriminator.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	for constant in omitted_constants {
		lines.push(format!(
			"\t\tif account.{} != {} {{",
			constant.field, constant.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	lines.push("\t\tOk(account)".to_string());
	lines.push("\t}".to_string());
	lines
}

fn render_compact_account_helpers(
	account_name: &str,
	discriminator: Option<&super::discriminator::DiscriminatorInfo>,
	omitted_constants: &[super::discriminator::OmittedConstantInfo],
) -> Vec<String> {
	let ref_name = format!("{account_name}Ref");
	let patch_name = format!("{account_name}Patch");
	let mut lines = vec![
		"\tpub const HEADER_SIZE: usize = <Self as pina::PinaPodCompact>::HEADER_SIZE;".to_string(),
		String::new(),
		format!(
			"\tpub fn initialize(data: &mut [u8], patch: {patch_name}<'_>) -> Result<usize, \
			 solana_program_error::ProgramError> {{"
		),
	];
	let mut patch = "\t\tpatch".to_string();
	if let Some(discriminator) = discriminator {
		let _ = write!(patch, ".discriminator({})", discriminator.name);
	}
	for constant in omitted_constants {
		let _ = write!(patch, ".{}({})", constant.field, constant.name);
	}
	lines.push(format!("{patch}.initialize(data)"));
	lines.extend([
		"\t\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)".to_string(),
		"\t}".to_string(),
		String::new(),
		format!(
			"\tpub fn from_bytes(data: &[u8]) -> Result<{ref_name}<'_>, \
			 solana_program_error::ProgramError> {{"
		),
		format!("\t\tlet account = {ref_name}::new(data)"),
		"\t\t\t.map_err(|_| solana_program_error::ProgramError::InvalidAccountData)?;".to_string(),
	]);
	if let Some(discriminator) = discriminator {
		lines.push(format!(
			"\t\tif account.discriminator != {} {{",
			discriminator.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	for constant in omitted_constants {
		lines.push(format!(
			"\t\tif account.{} != {} {{",
			constant.field, constant.name
		));
		lines.push(
			"\t\t\treturn Err(solana_program_error::ProgramError::InvalidAccountData);".to_string(),
		);
		lines.push("\t\t}".to_string());
	}
	lines.extend(["\t\tOk(account)".to_string(), "\t}".to_string()]);
	lines
}

fn render_account_pda_helpers(
	account_name: &str,
	pda: &PdaNode,
	primary_program_const: &str,
) -> Result<Vec<String>> {
	let mut params = Vec::new();
	let mut seed_exprs = Vec::new();

	for seed in &pda.seeds {
		match seed {
			PdaSeedNode::Variable(variable) => {
				let seed_name = snake(variable.name.as_ref());
				let context = format!("PDA `{}` variable seed `{seed_name}`", pda.name.as_ref());
				let (param_type, seed_expr) =
					render_variable_seed_parameter(&seed_name, &variable.r#type, &context)?;

				params.push(format!("{seed_name}: {param_type}"));
				seed_exprs.push(seed_expr);
			}
			PdaSeedNode::Constant(constant) => {
				let context = format!("PDA `{}` constant seed", pda.name.as_ref());
				seed_exprs.push(render_constant_seed_expression(
					&constant.r#type,
					&constant.value,
					&context,
					primary_program_const,
				)?);
			}
		}
	}

	let mut lines = Vec::new();
	lines.push(format!("impl {account_name} {{"));
	lines.push(format!(
		"\tpub fn find_pda({}) -> (solana_pubkey::Pubkey, u8) {{",
		params.join(", ")
	));
	lines.push("\t\tsolana_pubkey::Pubkey::find_program_address(".to_string());
	lines.push("\t\t\t&[".to_string());

	for seed_expr in &seed_exprs {
		lines.push(format!("\t\t\t\t{seed_expr},"));
	}

	lines.push("\t\t\t],".to_string());
	lines.push(format!("\t\t\t&crate::{primary_program_const},"));
	lines.push("\t\t)".to_string());
	lines.push("\t}".to_string());
	lines.push(String::new());

	let mut create_params = params.clone();
	create_params.push("bump: u8".to_string());
	lines.push(format!(
		"\tpub fn create_pda({}) -> Result<solana_pubkey::Pubkey, solana_pubkey::PubkeyError> {{",
		create_params.join(", ")
	));
	lines.push("\t\tsolana_pubkey::Pubkey::create_program_address(".to_string());
	lines.push("\t\t\t&[".to_string());

	for seed_expr in &seed_exprs {
		lines.push(format!("\t\t\t\t{seed_expr},"));
	}

	lines.push("\t\t\t\t&[bump],".to_string());
	lines.push("\t\t\t],".to_string());
	lines.push(format!("\t\t\t&crate::{primary_program_const},"));
	lines.push("\t\t)".to_string());
	lines.push("\t}".to_string());
	lines.push("}".to_string());

	Ok(lines)
}

/// Migration envelope facts for one account, when it is migratable.
pub(crate) struct MigrationEnvelope {
	pub(crate) module_name: String,
	pub(crate) version: u64,
	pub(crate) version_offset: usize,
	pub(crate) version_bytes: usize,
	pub(crate) discriminator: Vec<u8>,
}

/// Extract the `[discriminator, migrationVersion]` envelope from an account.
///
/// Returns `None` unless the account's first two fields carry numeric
/// defaults under exactly those names.
pub(crate) fn migration_envelope(account: &AccountNode) -> Option<MigrationEnvelope> {
	let fields = &account.data.get_nested_type_node().fields;
	let mut envelope = fields.iter().take(2).filter_map(|field| {
		let default_value = field.default_value.as_ref().as_ref()?;
		let kind = match field.name.as_ref() {
			"discriminator" => "discriminator",
			"migrationVersion" => "migrationVersion",
			_ => return None,
		};
		Some((kind, field.r#type.as_ref(), default_value))
	});

	let number_facts = |field_type: &TypeNode, default_value: &ValueNode| -> Option<(u64, usize)> {
		let ValueNode::Number(number_value) = default_value else {
			return None;
		};
		let TypeNode::Number(number_type) = field_type else {
			return None;
		};
		let width = match number_type.format {
			NumberFormat::U8 => 1,
			NumberFormat::U16 => 2,
			NumberFormat::U32 => 4,
			NumberFormat::U64 => 8,
			_ => return None,
		};
		let Number::UnsignedInteger(value) = number_value.number else {
			return None;
		};
		Some((value, width))
	};

	let Some(("discriminator", field_type, default_value)) = envelope.next() else {
		return None;
	};
	let discriminator = number_facts(field_type, default_value)?;
	let Some(("migrationVersion", field_type, default_value)) = envelope.next() else {
		return None;
	};
	let version = number_facts(field_type, default_value)?;

	Some(MigrationEnvelope {
		module_name: snake(account.name.as_ref()),
		version: version.0,
		version_offset: discriminator.1,
		version_bytes: version.1,
		discriminator: discriminator.0.to_le_bytes()[..discriminator.1].to_vec(),
	})
}

/// Render the per-account stale-bytes check for one migratable account.
pub(crate) fn render_needs_migration(envelope: &MigrationEnvelope) -> String {
	let constant = format!(
		"{}_MIGRATION_VERSION",
		envelope.module_name.to_shouty_snake_case()
	);
	let module = &envelope.module_name;
	let header = envelope.version_offset + envelope.version_bytes;
	let version_end = header;
	let mut conditions = Vec::new();
	for (index, byte) in envelope.discriminator.iter().enumerate() {
		conditions.push(format!("data[{index}] == {byte}"));
	}
	let conditions = conditions.join("\n\t\t\t&& ");

	format!(
		"\n/// Whether raw account bytes are stale for this contract: the envelope names this \
		 account's discriminator and carries a version older than\n/// [`{constant}`]. Current or \
		 foreign bytes return false; decoding explains the difference.\npub fn \
		 {module}_needs_migration(data: &[u8]) -> bool {{\n\tdata.len() >= {header}\n\t\t\t&& \
		 {conditions}\n\t\t\t&& {{\n\t\t\t\tlet mut version = [0_u8; \
		 8];\n\t\t\t\tversion[..{vb}]\n\t\t\t\t\t.copy_from_slice(&data[{vo}..{ve}]);\n\t\t\t\t		 \
		 u64::from_le_bytes(version) < {version}\n\t\t\t}}\n}}\n",
		constant = constant,
		module = module,
		header = header,
		conditions = conditions,
		vb = envelope.version_bytes,
		vo = envelope.version_offset,
		ve = version_end,
		version = envelope.version,
	)
}
/// Render the direction-aware decode API and contract tests for one fixed
/// migratable account.
fn render_try_from_bytes(envelope: &MigrationEnvelope) -> String {
	let account = pascal(&envelope.module_name);
	let zc_name = format!("{account}Zc");
	let error_enum = format!("{account}VersionError");
	let version_constant = format!(
		"{}_MIGRATION_VERSION",
		envelope.module_name.to_shouty_snake_case()
	);
	let discriminator_constant = format!(
		"{}_DISCRIMINATOR",
		envelope.module_name.to_shouty_snake_case()
	);
	let version_type = match envelope.version_bytes {
		2 => "u16",
		4 => "u32",
		8 => "u64",
		_ => "u8",
	};
	let version = envelope.version;
	let version_plus_one = version + 1;
	let version_offset = envelope.version_offset;
	let version_end = version_offset + envelope.version_bytes;
	let module = &envelope.module_name;
	let discriminator_len = envelope.discriminator.len();
	let discriminator_bytes = envelope
		.discriminator
		.iter()
		.map(u8::to_string)
		.collect::<Vec<_>>()
		.join(", ");
	let invalid_message = format!("invalid {account} account data");
	let stale_message = format!(
		"migration version mismatch: expected {version}, received 0 (the data predates this \
		 client; migrate it by sending a transaction to the program, or decode it with a client \
		 generated from an older IDL)"
	);
	let future_message = format!(
		"migration version mismatch: expected {version}, received {version_plus_one} (the data \
		 was written by a newer program; upgrade this client)"
	);

	let stale_hint = "the data predates this client; migrate it by sending a transaction to the \
	                  program, or decode it with a client generated from an older IDL";
	let future_hint = "the data was written by a newer program; upgrade this client";

	let mut lines = Vec::new();
	lines.push(String::new());
	lines.push(format!(
		"/// Why `{account}::try_from_bytes` rejected account bytes."
	));
	lines.push("#[derive(Clone, Copy, Debug, PartialEq, Eq)]".to_owned());
	lines.push(format!("pub enum {error_enum} {{"));
	lines.push("\t/// The bytes do not decode as this account's layout at all.".to_owned());
	lines.push("\tInvalidData,".to_owned());
	lines.push(
		"\t/// The envelope names this account but the stored version predates this client: \
		 migrate the account on-chain, then retry."
			.to_owned(),
	);
	lines.push(format!("\tStale {{ stored: {version_type} }},"));
	lines.push(
		"\t/// The envelope names this account but the stored version is newer than this client's \
		 schema: upgrade this client."
			.to_owned(),
	);
	lines.push(format!("\tFuture {{ stored: {version_type} }},"));
	lines.push("}".to_owned());
	lines.push(String::new());
	lines.push(format!("impl core::fmt::Display for {error_enum} {{"));
	lines.push(
		"\tfn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {".to_owned(),
	);
	lines.push("\t\tmatch self {".to_owned());
	lines.push(format!(
		"\t\t\tSelf::InvalidData => write!(f, \"{invalid_message}\"),"
	));
	lines.push("\t\t\tSelf::Stale { stored } => write!(".to_owned());
	lines.push("\t\t\t\tf,".to_owned());
	lines.push(format!(
		"\t\t\t\t\"migration version mismatch: expected {version}, received {{stored}} \
		 ({stale_hint})\""
	));
	lines.push("\t\t\t),".to_owned());
	lines.push("\t\t\tSelf::Future { stored } => write!(".to_owned());
	lines.push("\t\t\t\tf,".to_owned());
	lines.push(format!(
		"\t\t\t\t\"migration version mismatch: expected {version}, received {{stored}} \
		 ({future_hint})\""
	));
	lines.push("\t\t\t),".to_owned());
	lines.push("\t\t}".to_owned());
	lines.push("\t}".to_owned());
	lines.push("}".to_owned());
	lines.push(String::new());
	lines.push(format!("impl {account} {{"));
	lines.push(format!(
		"\t/// Decodes current-version bytes and tells stale envelopes (migrate the account) \
		 apart from future ones (upgrade this client). The failure message mirrors the generated \
		 JavaScript decoder. For the strict current-only convenience returning `ProgramError`, \
		 see [`{account}::from_bytes`]."
	));
	lines.push("\tpub fn try_from_bytes(".to_owned());
	lines.push("\t\tdata: &[u8],".to_owned());
	lines.push(format!("\t) -> Result<&{zc_name}, {error_enum}> {{"));
	lines.push(format!(
		"\t\tlet account = <Self as pina::PinaPodFixed>::read_exact(data)\n\t\t\t.map_err(|_| \
		 {error_enum}::InvalidData)?;"
	));
	lines.push(format!(
		"\t\tif account.discriminator != {discriminator_constant} {{"
	));
	lines.push(format!("\t\t\treturn Err({error_enum}::InvalidData);"));
	lines.push("\t\t}".to_owned());
	lines.push(format!(
		"\t\tif account.migration_version < {version_constant} {{"
	));
	lines.push(format!(
		"\t\t\treturn Err({error_enum}::Stale {{ stored: account.migration_version }});"
	));
	lines.push("\t\t}".to_owned());
	lines.push(format!(
		"\t\tif account.migration_version > {version_constant} {{"
	));
	lines.push(format!(
		"\t\t\treturn Err({error_enum}::Future {{ stored: account.migration_version }});"
	));
	lines.push("\t\t}".to_owned());
	lines.push("\t\tOk(account)".to_owned());
	lines.push("\t}".to_owned());
	lines.push("}".to_owned());
	lines.push(String::new());
	lines.push("#[cfg(test)]".to_owned());
	lines.push(format!("mod {module}_version_error_tests {{"));
	lines.push("\tuse super::*;".to_owned());
	lines.push(String::new());
	lines.push(format!(
		"\tfn envelope(version: {version_type}) -> Vec<u8> {{"
	));
	lines.push(format!(
		"\t\tlet mut data = vec![0_u8; core::mem::size_of::<{zc_name}>()];"
	));
	lines.push(format!(
		"\t\tdata[..{discriminator_len}].copy_from_slice(&[{discriminator_bytes}]);"
	));
	lines.push(format!(
		"\t\tdata[{version_offset}..{version_end}].copy_from_slice(&version.to_le_bytes());"
	));
	lines.push("\t\tdata".to_owned());
	lines.push("\t}".to_owned());
	lines.push(String::new());
	lines.push("\t#[test]".to_owned());
	lines.push("\tfn stale_and_future_versions_are_distinguishable() {".to_owned());
	lines.push(format!(
		"\t\tlet error = {account}::try_from_bytes(&envelope(0 as \
		 {version_type})).err().expect(\"a stale envelope must fail\");\n\t\tassert_eq!(error, \
		 {error_enum}::Stale {{ stored: 0 }});"
	));
	lines.push(format!(
		"\t\tassert_eq!({error_enum}::Stale {{ stored: 0 }}.to_string(), {stale_message:?});"
	));
	lines.push(format!(
		"\t\tlet error = {account}::try_from_bytes(&envelope({version_plus_one} as \
		 {version_type})).err().expect(\"a future envelope must fail\");\n\t\tassert_eq!(error, \
		 {error_enum}::Future {{ stored: {version_plus_one} }});"
	));
	lines.push(format!(
		"\t\tassert_eq!({error_enum}::Future {{ stored: {version_plus_one} }}.to_string(), \
		 {future_message:?});"
	));
	lines.push(format!(
		"\t\tassert!(\n\t\t\t{account}::try_from_bytes(&envelope({version} as \
		 {version_type})).is_ok(),\n\t\t\t\"the current version must decode\",\n\t\t);"
	));
	lines.push("\t}".to_owned());
	lines.push("}".to_owned());
	lines.join("\n")
}
